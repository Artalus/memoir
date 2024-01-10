use std::collections::VecDeque;
use std::io::{BufReader, ErrorKind, Result};
use std::sync::{Arc, Mutex};
use std::sync::mpsc::{Sender, Receiver, RecvTimeoutError};
use std::thread;
use std::time::Duration;

use interprocess::local_socket as ipc;

use crate::ipc_common::{socket_name, Signals};
use crate::process::{CurrentProcesses, list_processes};

type ProcessHistory = Arc<Mutex<VecDeque<CurrentProcesses>>>;
const HISTORY_CAPACITY: usize = 3600;

pub fn run_daemon() {
    let history = Arc::new(Mutex::new(VecDeque::with_capacity(HISTORY_CAPACITY)));

    let (snd, rcv) = std::sync::mpsc::channel();
    let ipce = fork_ipc(snd, history.clone());
    let ipc = match ipce {
        Err(e) => {
            println!("Error: failed to setup IPC: {}", e);
            return;
        },
        Ok(i) => i,
    };
    run_process_list_daemon(rcv, history.clone());
    ipc.join().unwrap();
}

pub fn fork_ipc(finish_snd: Sender<()>, process_history: ProcessHistory) -> Result<thread::JoinHandle<()>> {
    let listener = match ipc::LocalSocketListener::bind(socket_name()) {
        Err(e) if e.kind() == ErrorKind::AddrInUse => {
            // TODO: detect if other instance of memoir is actually running
            eprintln!(
                "Error: could not start server because the socket file is occupied. \
                Check if {} is in use by another memoir process and try again.",
                socket_name(),
            );
            return Err(e);
        }
        Err(e) => {
            eprint!("Error: could not start server: {}", e);
            return Err(e);
        }
        Ok(x) => x,
    };
    println!("Server running at {}", socket_name());
    let handle = thread::spawn(move || ipc_listen(finish_snd, listener, process_history));
    Ok(handle)
}

pub fn run_process_list_daemon(finish_rcv: Receiver<()>, history: ProcessHistory) {
    // 1 second wait between process polls is done via recv() timeout
    while listing_should_continue(&finish_rcv, Duration::new(1, 0)) {
        let mut locked = history.lock().unwrap();
        if locked.len() >= HISTORY_CAPACITY {
            locked.pop_front();
        }
        locked.push_back(list_processes());
    }
}

fn listing_should_continue(finish_rcv: &Receiver<()>, timeout: Duration) -> bool {
    match finish_rcv.recv_timeout(timeout) {
        Ok(_) => false,
        Err(RecvTimeoutError::Timeout) => {
            true
        },
        Err(RecvTimeoutError::Disconnected) => {
            eprintln!("Error: receiver's counterpart disconnected!");
            false
        },
    }
}

fn ipc_listen(finish_snd: Sender<()>, listener: ipc::LocalSocketListener, _history: ProcessHistory) {
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
        if let Err(e) = reader.get_mut().write_all(&Signals::Ack.as_cmdline()) {
            eprintln!("Error: write_all failed: {}", e);
            break;
        }

        // Print out the result, getting the newline for free!
        eprintln!("Client sent: '{}'", buffer);

        // Let's add an exit condition to shut the server down gracefully.
        if buffer.as_bytes() == Signals::Stop.as_cmdline() {
            finish_snd.send(()).expect("Error: could not send stop signal");
            break;
        }

        // Clear the buffer so that the next iteration will display new data instead of messages
        // stacking on top of one another.
        buffer.clear();
    }
    println!("daemon finished");
}

fn handle_ipc_connection_error(
    conn: Result<ipc::LocalSocketStream>,
) -> Option<ipc::LocalSocketStream> {
    match conn {
        Ok(c) => Some(c),
        Err(e) => {
            eprintln!("Incoming connection failed: {}", e);
            None
        }
    }
}
