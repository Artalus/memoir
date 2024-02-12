use anyhow::Context;
use interprocess::local_socket::{LocalSocketStream, NameTypeSupport};
use serde::{Deserialize, Serialize};

pub fn socket_name() -> String {
    use NameTypeSupport::*;
    match NameTypeSupport::query() {
        OnlyPaths => String::from("/tmp/memoirrs.sock"),
        OnlyNamespaced | Both => String::from("@memoirrs.sock"),
    }
}
#[derive(Serialize, Deserialize, Debug)]
pub enum SaveTo {
    File { name: String },
    Stdout,
}
#[derive(Serialize, Deserialize, Debug)]
pub enum Signal {
    Ack,
    Error { explanation: String },
    Stop,
    Ping,
    Save { to: SaveTo, time_sec: Option<usize> },
    Output { output: String },
}
impl Signal {
    pub fn feed_into(self, into: &mut LocalSocketStream) -> anyhow::Result<()> {
        let writer = std::io::BufWriter::new(into);
        ciborium::into_writer(&self, writer).context("Failed to write signal to socket")
    }

    pub fn read_from(from: &mut LocalSocketStream) -> anyhow::Result<Signal> {
        let reader = std::io::BufReader::new(from);
        ciborium::from_reader(reader).context("Failed to read signal from socket")
    }
}
