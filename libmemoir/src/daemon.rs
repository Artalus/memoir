use std::borrow::BorrowMut;
use std::collections::{HashSet, VecDeque};
use std::io::ErrorKind;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use interprocess::local_socket::{LocalSocketListener, LocalSocketStream};

use crate::csvdump;
use crate::ipc_common::{socket_name, SaveTo, Signal};
use crate::process::{list_processes, CurrentProcesses, Process};

type ProcessHistory = Arc<Mutex<VecDeque<CurrentProcesses>>>;
const CLEANUP_INTERVAL: usize = 100;

pub enum PingResult {
    DaemonExists,
    DaemonNotFound,
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
        match LocalSocketListener::bind(socket_name.clone()) {
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
    let mut conn = LocalSocketStream::connect(socket_name.clone())
        .context(format!("Unable to connect to socket {}", socket_name))?;

    Signal::Ping
        .feed_into(&mut conn)
        .context("Unable to send ping to daemon")?;

    match Signal::read_from(&mut conn) {
        Ok(Signal::Ack) => Ok(PingResult::DaemonExists),
        Err(e) => Err(e).context("Unable to receive pong from daemon"),
        x => Err(anyhow!("Unexpected response from daemon: {:?}", x)),
    }
}

fn fork_ipc(
    finish_snd: Sender<()>,
    process_history: ProcessHistory,
) -> Result<thread::JoinHandle<Result<()>>> {
    let listener = match LocalSocketListener::bind(socket_name()) {
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
    listener: LocalSocketListener,
    history: ProcessHistory,
) -> Result<()> {
    println!("daemon started");

    for mut conn in listener.incoming().filter_map(handle_ipc_connection_error) {
        println!("Incoming connection!");
        let received =
            Signal::read_from(&mut conn).context("Could not read signal from connection");
        if received.is_err() {
            let mut uw = received.unwrap_err();
            let feed_result = Signal::Error {
                explanation: format!("{uw:?}"),
            }
            .feed_into(&mut conn)
            .context("Also could not respond with error to connection");
            if feed_result.is_err() {
                uw = uw.context(feed_result.unwrap_err());
            }
            return Err(uw);
        } else {
            Signal::Ack
                .feed_into(&mut conn)
                .context("Could not respond with ack to connection")?;
        }
        match received.unwrap() {
            Signal::Stop => {
                finish_snd
                    .send(())
                    .context("Error: could not send stop signal")?;
                break;
            }
            Signal::Save { to, time_sec } => match to {
                SaveTo::File { name } => {
                    eprintln!("Saving current process info to {:?}...", name);
                    csvdump::save_to_file(&history.lock().unwrap(), &PathBuf::from(name), time_sec)
                        .context("Could not dump process history to CSV file")?;
                }
                SaveTo::Stdout => {
                    let hss = history.lock().unwrap();
                    let mut iterator = crate::csvdump::iterator_on_buffer(hss.iter(), time_sec);

                    // this is so wrong on so many levels, but after 8 hours i just give up on rust
                    unsafe fn very_bad_function<T>(reference: &T) -> &mut T {
                        let const_ptr = reference as *const T;
                        let mut_ptr = const_ptr as *mut T;
                        &mut *mut_ptr
                    }

                    // need a frolicking raw `loop`, as `for x in i` will cause Rust to
                    // `into_iter(i)`, moving its contents away and making .writer inaccessible
                    loop {
                        match iterator.next() {
                            None => break,
                            Some(chunk) => {
                                match chunk {
                                    Ok(()) => {}
                                    Err(e) => {
                                        let res: Result<()> =
                                            Err(e).context("Iteration over process list failed");
                                        let feed = Signal::Error {
                                            e: res.unwrap_err(),
                                        }
                                        .feed_into(&mut conn)
                                        .context("Could not feed error into connection");
                                        match &feed {
                                            Ok(()) => {}
                                            Err(e) => {
                                                eprintln!("ERROR: {e:?}");
                                            }
                                        }
                                        feed?;
                                    }
                                };
                                let writer = iterator.writer.borrow_mut();
                                writer.flush()?;
                                let cursor = writer.get_ref();
                                let content = {
                                    let huher = cursor.get_ref();
                                    std::str::from_utf8(huher.as_slice()).unwrap().to_string()
                                };
                                println!("{}", content);
                                unsafe {
                                    let cursor2 = very_bad_function(cursor);
                                    cursor2.set_position(0);
                                };
                                if bad_stuff.is_some() {
                                    return bad_stuff.unwrap();
                                }
                            }
                        }
                    }
                    // for chunk in iterator {
                    //     let feed = Signal::Output {
                    //         output: std::str::from_utf8(buffer.as_slice()).unwrap().to_string(),
                    //     }
                    //     .feed_into(&mut conn)
                    //     .context("Could not feed dump into connection");
                    //     match feed {
                    //         Ok(()) => {}
                    //         Err(e) => {
                    //             eprintln!("Could not dump process history into connection");
                    //         }
                    //     }
                    // }
                }
            },
            x => {
                eprintln!("Unexpected signal: {x:?}");
            }
        }
    }
    println!("daemon finished");
    Ok(())
}

fn handle_ipc_connection_error(
    conn: std::io::Result<LocalSocketStream>,
) -> Option<LocalSocketStream> {
    match conn {
        Ok(c) => Some(c),
        Err(e) => {
            eprintln!("Incoming connection failed: {}", e);
            None
        }
    }
}
