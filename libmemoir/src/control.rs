use std::collections::{HashSet, VecDeque};

use anyhow::{anyhow, Context, Result};
use interprocess::local_socket::LocalSocketStream;

use crate::{
    daemon,
    ipc_common::{socket_name, SaveTo, Signal},
    process::{list_processes, Process},
};

/// Spawn a separate monitoring process, wait for it to successfully start and
/// exit immediately leaving it in background.
pub fn do_detach(history_capacity: usize) -> Result<()> {
    match daemon::check_socket_status() {
        Ok(daemon::PingResult::DaemonExists) => {
            eprintln!("Daemon already active.");
            return Ok(());
        }
        Ok(daemon::PingResult::DaemonNotFound) => {}
        Err(e) => return Err(e).context("Unexpected error during initial check"),
    }

    use std::process::Command;
    let exe = std::env::current_exe().context("Could not get current executable path")?;
    let mut command = Command::new(exe);
    command
        .args([
            "run",
            "--without-checks",
            "--history-size",
            &history_capacity.to_string(),
        ])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        command.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
    }
    let mut child = command
        .spawn()
        .context("Could not spawn child daemon process")?;
    let mut await_spawn_attempts = 0;
    let mut last_error: anyhow::Error;
    loop {
        await_spawn_attempts += 1;
        match child.try_wait() {
            Ok(None) => {} // process is running, all good
            Ok(Some(s)) => {
                return Err(anyhow!(
                    "Daemon exited with code {s}.\n\
                You can try to restart it without detaching to see the output:\n\
                \x20   memoirctl run"
                ))
            }
            Err(e) => {
                eprintln!("Error: could not check daemon exit status: {}", e);
                // do not exit though, as our main indicator is ping response
            }
        }

        // give child 1ms to init
        std::thread::sleep(std::time::Duration::new(0, 1_000_000));

        match communicate(Signal::Ping) {
            Ok(_) => break,
            Err(e) => {
                // errors are allowed to occur at first, if the daemon hadn't yet bound
                // the socket
                last_error = e
            }
        }
        if await_spawn_attempts == 5 {
            return Err(last_error).context("Daemon did not spawn after 5 attempts");
        }
        eprintln!("Daemon did not answer to ping, waiting...");
        std::thread::sleep(std::time::Duration::new(1, 0));
    }
    eprintln!("Daemon started.");
    Ok(())
}

/// Run the monitoring daemon, with one thread collecting process statistics and
/// another listening on a local socket for communication from other memoirctl.
pub fn do_run(as_daemon: bool, history_capacity: usize) -> Result<()> {
    if as_daemon {
        match daemon::check_socket_status() {
            Ok(daemon::PingResult::DaemonExists) => return Err(anyhow!("Daemon already active.")),
            Ok(daemon::PingResult::DaemonNotFound) => {}
            Err(e) => return Err(e).context("Unexpected error during initial check"),
        }
        let tmp = std::env::temp_dir();
        std::env::set_current_dir(&tmp).context(format!(
            "Could not change directory to temporary dir {:?}",
            tmp
        ))?;
    }
    daemon::run_daemon(history_capacity)
}

pub fn do_stop() -> Result<()> {
    communicate(Signal::Stop)
}

pub fn do_status() -> Result<()> {
    match daemon::check_socket_status() {
        Ok(daemon::PingResult::DaemonExists) => {
            eprintln!("Daemon active.");
            Ok(())
        }
        Ok(daemon::PingResult::DaemonNotFound) => Err(anyhow!("Daemon not running.")),
        Err(e) => Err(e).context("Unexpected error during ping"),
    }
}

pub fn do_once() -> Result<()> {
    let mut cache: HashSet<std::sync::Arc<Process>> = HashSet::with_capacity(1000);
    let lp = list_processes(&mut cache)?;
    let vd = VecDeque::from([lp]);
    let mut buffer = Vec::new();
    let writer = std::io::BufWriter::new(&mut buffer);
    crate::csvdump::save_to_stream(&vd, writer, None)
        .context("Could not dump process history to buffer")?;
    println!("{}", std::str::from_utf8(buffer.as_slice()).unwrap());
    Ok(())
}

pub fn do_save(to: &String, last: Option<usize>) -> Result<()> {
    let file = std::env::current_dir()
        .context("Could not get current directory")?
        .join(to);
    let parent = file.parent().unwrap();
    let parentname = parent.as_os_str();
    if !parent.exists() {
        return Err(anyhow!("Directory {:?} does not exist", &parentname));
    }
    let filename = file.into_os_string().into_string().unwrap();
    println!("-- requesting save to {:?}", filename);
    communicate(Signal::Save {
        to: SaveTo::File { name: filename },
        time_sec: last,
    })
}

pub fn do_dump(last: Option<usize>) -> Result<()> {
    println!("-- requesting dump");
    let mut conn = connect()?;
    send(
        Signal::Save {
            to: SaveTo::Stdout,
            time_sec: last,
        },
        &mut conn,
    )?;
    let r = receive(&mut conn)?;
    match r {
        Signal::Ack => {}
        x => return Err(anyhow!("Unexpected response signal #1 {x:?}")),
    }
    let r = receive(&mut conn)?;
    match r {
        Signal::Output { output } => {
            println!("{}", output);
        }
        x => return Err(anyhow!("Unexpected response signal #2 {x:?}")),
    }
    Ok(())
}

fn connect() -> anyhow::Result<LocalSocketStream> {
    LocalSocketStream::connect(socket_name()).context("Connection to server failed")
}

fn send(signal: Signal, conn: &mut LocalSocketStream) -> Result<()> {
    signal
        .feed_into(conn)
        .context("Writing signal to server failed")
}

fn receive(conn: &mut LocalSocketStream) -> Result<Signal> {
    let response = Signal::read_from(conn).context("Reading server response failed")?;
    Ok(response)
}

fn communicate(signal: Signal) -> Result<()> {
    let mut c = connect()?;
    send(signal, &mut c)?;
    match receive(&mut c) {
        Ok(s) => match s {
            Signal::Ack => Ok(()),
            Signal::Error => Err(anyhow!("Daemon returned error at communication")),
            x => Err(anyhow!("Unexpected response signal from daemon: {x:?}")),
        },
        Err(e) => Err(e).context("Could receive response from daemon"),
    }
}
