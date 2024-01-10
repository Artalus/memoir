use std::io::BufReader;

use interprocess::local_socket as ipc;

use crate::ipc_common::{socket_name, Signals};

pub fn run_control(args: Vec<String>) {
    if args[0] == "start" {
        crate::daemon::run_daemon();
        return;
    }

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

    // if args[0] == "stop" {
    //     do_stop(&mut reader, &mut buffer);
    // }
    match args[0].as_str() {
        "stop" => do_stop(&mut reader, &mut buffer),
        "ping" => do_ping(&mut reader, &mut buffer),
        _ => {
            eprintln!("Error: unknown command: {}", args[0]);
        }
    }
}

fn do_stop(reader: &mut BufReader<ipc::LocalSocketStream>, buffer: &mut String) {
    use std::io::prelude::*;

    // Write our message into the stream. This will finish either when the whole message has been
    // writen or if a write operation returns an error. (`.get_mut()` is to get the writer,
    // `BufReader` doesn't implement a pass-through `Write`.)
    eprintln!("-- write...");
    if let Err(e) = reader.get_mut().write_all(&Signals::Stop.as_cmdline()) {
        eprintln!("Error: write_all failed: {}", e);
        return;
    }
    eprintln!("-- read...");

    // We now employ the buffer we allocated prior and read until EOF, which the server will
    // similarly invoke with `.shutdown()`, verifying validity of UTF-8 on the fly.
    if let Err(e) = reader.read_line(buffer) {
        eprintln!("Error: read_line failed: {}", e);
        return;
    }
    println!("-- server answered: '{}'", buffer);

}

fn do_ping(reader: &mut BufReader<ipc::LocalSocketStream>, buffer: &mut String) {
    use std::io::prelude::*;

    // Write our message into the stream. This will finish either when the whole message has been
    // writen or if a write operation returns an error. (`.get_mut()` is to get the writer,
    // `BufReader` doesn't implement a pass-through `Write`.)
    eprintln!("-- write...");
    if let Err(e) = reader.get_mut().write_all(&Signals::Ping.as_cmdline()) {
        eprintln!("Error: write_all failed: {}", e);
        return;
    }
    eprintln!("-- read...");

    // We now employ the buffer we allocated prior and read until EOF, which the server will
    // similarly invoke with `.shutdown()`, verifying validity of UTF-8 on the fly.
    if let Err(e) = reader.read_line(buffer) {
        eprintln!("Error: read_line failed: {}", e);
        return;
    }
    println!("-- server answered: '{}'", buffer);
}
