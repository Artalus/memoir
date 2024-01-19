use std::collections::{HashSet, VecDeque};
use std::ffi::OsString;
use std::io::{BufReader, ErrorKind};
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use interprocess::local_socket as ipc;

use crate::csvdump::save_to_csv;
use crate::ipc_common::{socket_name, Signal};
use crate::process::{list_processes, CurrentProcesses, Process};

type ProcessHistory = Arc<Mutex<VecDeque<CurrentProcesses>>>;
const CLEANUP_INTERVAL: usize = 100;

pub enum PingResult {
    DaemonExists,
    DaemonNotFound,
    SocketOccupied,
}

/// Run a daemon-server listening to a LocalSocket. Blocks until the daemon is stopped.
pub fn run_daemon(history_capacity: usize) -> Result<()> {
    eprintln!("Using history capacity of {history_capacity} seconds");
    let history = Arc::new(Mutex::new(VecDeque::with_capacity(history_capacity)));

    let (snd, rcv) = std::sync::mpsc::channel();
    let ipc = fork_ipc(snd, history.clone()).context("Error: failed to setup IPC")?;
    run_process_list_daemon(rcv, history.clone(), history_capacity)?;
    ipc.join().unwrap()
}

pub fn check_socket_status() -> Result<PingResult> {
    let socket_name = socket_name();
    {
        match ipc::LocalSocketListener::bind(socket_name.clone()) {
            // if we could bind to socket, the daemon defo does not exist
            Ok(_) => return Ok(PingResult::DaemonNotFound),
            // if addr is in use, there might be a chance it is occupied by something alien
            Err(e) if e.kind() == ErrorKind::AddrInUse => {}
            // any other error might prevent us from binding to the socket later
            Err(e) => {
                return Err(e).context(format!("Unable to bind socket {}", socket_name));
            }
        }
    }
    // attempt to ping the socket to ensure it is occupied by our daemon
    let mut buffer = String::with_capacity(128);
    let conn = ipc::LocalSocketStream::connect(socket_name.clone())
        .context(format!("Unable to connect to socket {}", socket_name))?;
    let mut reader = BufReader::new(conn);

    // to access write_all/read_line
    use std::io::prelude::*;

    reader
        .get_mut()
        .write_all(&Signal::Ping.as_cmdline())
        .context("Unable to send ping to daemon")?;

    reader
        .read_line(&mut buffer)
        .context("Unable to receive pong from daemon")?;
    if buffer.as_bytes() == Signal::Ack.as_cmdline() {
        return Ok(PingResult::DaemonExists);
    }
    Ok(PingResult::SocketOccupied)
}

fn fork_ipc(
    finish_snd: Sender<()>,
    process_history: ProcessHistory,
) -> Result<thread::JoinHandle<Result<()>>> {
    let listener = match ipc::LocalSocketListener::bind(socket_name()) {
        Err(e) if e.kind() == ErrorKind::AddrInUse => {
            // TODO: detect if other instance of memoir is actually running
            eprintln!(
                "Error: could not start server because the socket file is occupied. \
                Check if {} is in use by another memoir process and try again.",
                socket_name(),
            );
            return Err(anyhow!(e));
        }
        Err(e) => {
            eprint!("Error: could not start server: {}", e);
            return Err(anyhow!(e));
        }
        Ok(x) => x,
    };
    println!("Server running at {}", socket_name());
    let handle = thread::spawn(move || ipc_listen(finish_snd, listener, process_history));
    Ok(handle)
}

pub fn run_process_list_daemon(
    finish_rcv: Receiver<()>,
    history: ProcessHistory,
    history_capacity: usize,
) -> Result<()> {
    let mut cache: HashSet<Arc<Process>> = HashSet::with_capacity(1000);
    let mut cleanup_tick = 0;
    // 1 second wait between process polls is done via recv() timeout
    while listing_should_continue(&finish_rcv, Duration::new(1, 0)) {
        cleanup_tick += 1;
        let mut locked = history.lock().unwrap();
        locked.push_back(list_processes(&mut cache)?);
        if locked.len() > history_capacity {
            locked.pop_front();
        }
        if cleanup_tick >= CLEANUP_INTERVAL {
            cleanup_tick = 0;
            cache.retain(|c| Arc::strong_count(c) > 1);
        }
    }
    Ok(())
}

fn listing_should_continue(finish_rcv: &Receiver<()>, timeout: Duration) -> bool {
    match finish_rcv.recv_timeout(timeout) {
        Ok(_) => false,
        Err(RecvTimeoutError::Timeout) => true,
        Err(RecvTimeoutError::Disconnected) => {
            eprintln!("Error: receiver's counterpart disconnected!");
            false
        }
    }
}

fn ipc_listen(
    finish_snd: Sender<()>,
    listener: ipc::LocalSocketListener,
    history: ProcessHistory,
) -> Result<()> {
    println!("daemon started");
    // Preemptively allocate a sizeable buffer for reading at a later moment. This size should be
    // enough and should be easy to find for the allocator. Since we only have one concurrent
    // client, there's no need to reallocate the buffer repeatedly.
    let mut buffer = String::with_capacity(128);

    for conn in listener.incoming().filter_map(handle_ipc_connection_error) {
        use std::io::prelude::*;

        // Wrap the connection into a buffered reader right away
        // so that we could read a single line out of it.
        let mut reader = BufReader::new(conn);
        println!("Incoming connection!");

        // Since our client example writes first, the server should read a line and only then send a
        // response. Otherwise, because reading and writing on a connection cannot be simultaneous
        // without threads or async, we can deadlock the two processes by having both sides wait for
        // the write buffer to be emptied by the other.
        if let Err(e) = reader.read_line(&mut buffer) {
            eprintln!("Error: read_line failed: {}", e);
            break;
        }

        // Now that the read has come through and the client is waiting on the server's write, do
        // it. (`.get_mut()` is to get the writer, `BufReader` doesn't implement a pass-through
        // `Write`.)
        if let Err(e) = reader.get_mut().write_all(&Signal::Ack.as_cmdline()) {
            eprintln!("Error: write_all failed: {}", e);
            break;
        }

        // Print out the result, getting the newline for free!
        eprintln!("Client sent: '{}'", buffer);

        // Let's add an exit condition to shut the server down gracefully.
        if buffer.as_bytes() == Signal::Stop.as_cmdline() {
            finish_snd
                .send(())
                .context("Error: could not send stop signal")?;
            break;
        }
        if buffer.as_bytes() == Signal::Save.as_cmdline() {
            let mut len_buffer: [u8; 8] = [0; 8];
            reader
                .read_exact(&mut len_buffer)
                .context("Error: could not read save argument length")?;
            let arg_len = u64::from_be_bytes(len_buffer);
            let mut arg_buffer: Vec<u8> = vec![0; arg_len as usize];
            reader
                .read_exact(&mut arg_buffer)
                .context("Error: could not read save argument")?;
            let arg = unsafe { OsString::from_encoded_bytes_unchecked(arg_buffer) };
            reader
                .get_mut()
                .write_all(&Signal::Ack.as_cmdline())
                .context("Error: could not write ack on arg")?;

            eprintln!("Saving current process info to {:?}...", arg);
            save_to_csv(&history.lock().unwrap(), &PathBuf::from(arg))
                .context("Could not dump process history to CSV")?;
        }

        // Clear the buffer so that the next iteration will display new data instead of messages
        // stacking on top of one another.
        buffer.clear();
    }
    println!("daemon finished");
    Ok(())
}

fn handle_ipc_connection_error(
    conn: std::io::Result<ipc::LocalSocketStream>,
) -> Option<ipc::LocalSocketStream> {
    match conn {
        Ok(c) => Some(c),
        Err(e) => {
            eprintln!("Incoming connection failed: {}", e);
            None
        }
    }
}
