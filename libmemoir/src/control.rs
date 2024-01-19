use std::{collections::HashSet, io::BufReader};

use anyhow::{anyhow, Context};
use interprocess::local_socket as ipc;

use crate::{
    daemon,
    ipc_common::{socket_name, Signal},
    process::{list_processes, Process},
};

type Result = anyhow::Result<()>;

/// Spawn a separate monitoring process, wait for it to successfully start and
/// exit immediately leaving it in background.
pub fn do_detach(history_capacity: usize) -> Result {
    match daemon::check_socket_status() {
        Ok(daemon::PingResult::DaemonExists) => {
            eprintln!("Daemon already active.");
            return Ok(());
        }
        Ok(daemon::PingResult::DaemonNotFound) => {}
        Ok(daemon::PingResult::SocketOccupied) => {
            return Err(anyhow!("Socket bound to some other program."))
        }
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
        const DETACHED_PROCESS: u32 = 0x000_000_08;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x000_002_00;
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

        match communicate(Signal::Ping, b"") {
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
pub fn do_run(as_daemon: bool, history_capacity: usize) -> Result {
    if as_daemon {
        match daemon::check_socket_status() {
            Ok(daemon::PingResult::DaemonExists) => return Err(anyhow!("Daemon already active.")),
            Ok(daemon::PingResult::DaemonNotFound) => {}
            Ok(daemon::PingResult::SocketOccupied) => {
                return Err(anyhow!("Socket bound to some other program."))
            }
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

pub fn do_stop() -> Result {
    communicate(Signal::Stop, b"")
}

pub fn do_status() -> Result {
    // TODO: this is more complex than just ping, rename to `status` or smth
    match daemon::check_socket_status() {
        Ok(daemon::PingResult::DaemonExists) => {
            eprintln!("Daemon active.");
            Ok(())
        }
        Ok(daemon::PingResult::DaemonNotFound) => Err(anyhow!("Daemon not running.")),
        Ok(daemon::PingResult::SocketOccupied) => {
            Err(anyhow!("Socket bound to some other program."))
        }
        Err(e) => Err(e).context("Unexpected error during ping"),
    }
}

pub fn do_once() -> Result {
    let mut cache: HashSet<std::sync::Arc<Process>> = HashSet::with_capacity(1000);
    let lp = list_processes(&mut cache)?;
    println!("Processes: {}", lp);
    Ok(())
}

pub fn do_save(to: &String) -> Result {
    let file = std::env::current_dir()
        .context("Could not get current directory")?
        .join(&to);
    let parent = file.parent().unwrap();
    let parentname = parent.as_os_str().to_os_string();
    if !parent.exists() {
        return Err(anyhow!("Directory {:?} does not exist", &parentname));
    }
    let filename = file.as_os_str();
    println!("-- requesting save to {:?}", filename);
    communicate(Signal::Save, filename.as_encoded_bytes())
}

fn communicate(signal: Signal, arg: &[u8]) -> Result {
    let mut buffer = String::with_capacity(128);

    // block until server accepts connection, failing immediately if server hasn't started yet
    let conn =
        ipc::LocalSocketStream::connect(socket_name()).context("Connection to server failed")?;
    let mut reader = BufReader::new(conn);

    // to access write_all/read_line
    use std::io::prelude::*;

    reader
        .get_mut()
        .write_all(&signal.as_cmdline())
        .context("Writing signal to server failed")?;

    // We now employ the buffer we allocated prior and read until EOF, which the server will
    // similarly invoke with `.shutdown()`, verifying validity of UTF-8 on the fly.
    reader
        .read_line(&mut buffer)
        .context("Reading server response failed")?;
    // if buffer.as_bytes() == Signal::Ack.as_cmdline() {
    println!("-- server answered: '{}'", buffer);

    if !arg.is_empty() {
        let arg_length = (arg.len() as u64).to_be_bytes();
        reader
            .get_mut()
            .write(&arg_length)
            .context("Could not write argument length to server")?;
        reader
            .get_mut()
            .write(arg)
            .context("Could not write argument to server")?;
        reader
            .read_line(&mut buffer)
            .context("Reading server response to arg failed")?;
        // if buffer.as_bytes() == Signal::Ack.as_cmdline() {
        println!("-- server answered on arg: '{}'", buffer);
    }
    Ok(())
}
