use std::{collections::HashSet, io::BufReader};

use anyhow::Context;
use interprocess::local_socket as ipc;

use crate::{
    ipc_common::{socket_name, Signal},
    process::{list_processes, Process},
};

type Result = anyhow::Result<()>;

pub fn run_control(args: Vec<String>) -> Result {
    match args[0].as_str() {
        "once" => Ok(do_once()),
        "start" => Ok(do_start()),
        "stop" => do_stop(),
        "ping" => do_ping(),
        "save" => do_save(&args[1..]),
        _ => Err(anyhow::Error::msg(format!("Unknown command: {}", args[0]))),
    }
}

fn do_start() {
    crate::daemon::run_daemon();
}

fn do_stop() -> Result {
    communicate(Signal::Stop, b"")
}

fn do_ping() -> Result {
    communicate(Signal::Ping, b"")
}

fn do_once() {
    let mut cache: HashSet<std::sync::Arc<Process>> = HashSet::with_capacity(1000);
    let lp = list_processes(&mut cache);
    println!("Processes: {}", lp);
}

fn do_save(args: &[String]) -> Result {
    if args.len() < 1 {
        eprintln!("Error: save requires a file name");
    }
    let file = std::env::current_dir()
        .context("Could not get current directory")?
        .join(&args[0]);
        // .context(format!("Failed to resolve file name for '{}'", &args[0]))?;
    let parent = file.parent().unwrap();
    let parentname = parent.as_os_str().to_os_string();
    if ! parent.exists() {
        return Err(anyhow::Error::msg(format!("Directory {:?} does not exist", &parentname)));
    }
    let filename = file.as_os_str();
    println!("-- requesting save to {:?}", filename);
    communicate(Signal::Save, &filename.as_encoded_bytes())
}

fn communicate(signal: Signal, arg: &[u8]) -> Result {
    let mut buffer = String::with_capacity(128);

    // block until server accepts connection, failing immediately if server hasn't started yet
    let conn = ipc::LocalSocketStream::connect(socket_name()).context("Connection to server failed")?;
    let mut reader = BufReader::new(conn);

    // to access write_all/read_line
    use std::io::prelude::*;

    reader.get_mut().write_all(&signal.as_cmdline()).context("Writing signal to server failed")?;

    // We now employ the buffer we allocated prior and read until EOF, which the server will
    // similarly invoke with `.shutdown()`, verifying validity of UTF-8 on the fly.
    reader.read_line(&mut buffer).context("Reading server response failed")?;
    // if buffer.as_bytes() == Signal::Ack.as_cmdline() {
    println!("-- server answered: '{}'", buffer);

    if arg.len() > 0 {
        let arg_length = (arg.len() as u64).to_be_bytes();
        reader.get_mut().write(&arg_length).context("Could not write argument length to server")?;
        reader.get_mut().write(arg).context("Could not write argument to server")?;
        reader.read_line(&mut buffer).context("Reading server response failed")?;
        // if buffer.as_bytes() == Signal::Ack.as_cmdline() {
        println!("-- server answered on arg: '{}'", buffer);
    }
    Ok(())
}
