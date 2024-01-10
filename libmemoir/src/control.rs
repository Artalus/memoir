use std::io::BufReader;

use interprocess::local_socket as ipc;

use crate::{ipc_common::{socket_name, Signals}, process::list_processes};

pub fn run_control(args: Vec<String>) {
    match args[0].as_str() {
        "once" => do_once(),
        "start" => do_start(),
        "stop" => do_stop(),
        "ping" => do_ping(),
        _ => {
            eprintln!("Error: unknown command: {}", args[0]);
        }
    }
}

fn do_start() {
    crate::daemon::run_daemon();
}

fn do_stop() {
    communicate(Signals::Stop);
}

fn do_ping() {
    communicate(Signals::Ping);
}

fn do_once() {
    let lp = list_processes();
    println!("Processes: {}", lp);
}

fn communicate(signal: Signals) {
    let mut buffer = String::with_capacity(128);

    // Create our connection. This will block until the server accepts our connection, but will fail
    // immediately if the server hasn't even started yet; somewhat similar to how happens with TCP,
    // where connecting to a port that's not bound to any server will send a "connection refused"
    // response, but that will take twice the ping, the roundtrip time, to reach the client.
    let conn = match ipc::LocalSocketStream::connect(socket_name()) {
        Err(e) => {
            eprintln!("Error: connect failed: {}", e);
            return;
        },
        Ok(c) => c,
    };
    // Wrap it into a buffered reader right away so that we could read a single line out of it.
    let mut reader = BufReader::new(conn);

    use std::io::prelude::*;

    // Write our message into the stream. This will finish either when the whole message has been
    // writen or if a write operation returns an error. (`.get_mut()` is to get the writer,
    // `BufReader` doesn't implement a pass-through `Write`.)
    eprintln!("-- write...");
    if let Err(e) = reader.get_mut().write_all(&signal.as_cmdline()) {
        eprintln!("Error: write_all failed: {}", e);
        return;
    }
    eprintln!("-- read...");

    // We now employ the buffer we allocated prior and read until EOF, which the server will
    // similarly invoke with `.shutdown()`, verifying validity of UTF-8 on the fly.
    if let Err(e) = reader.read_line(&mut buffer) {
        eprintln!("Error: read_line failed: {}", e);
        return;
    }
    println!("-- server answered: '{}'", buffer);
}
