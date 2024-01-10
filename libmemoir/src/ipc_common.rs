use interprocess::local_socket as ipc;

pub fn socket_name() -> String {
    use ipc::NameTypeSupport::*;
    match ipc::NameTypeSupport::query() {
        OnlyPaths => String::from("/tmp/memoirrs.sock"),
        OnlyNamespaced | Both => String::from("@memoirrs.sock"),
    }
}

pub enum Signals {
    Ack,
    Stop,
    Ping,
}

impl std::fmt::Display for Signals {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Signals::Ack => write!(f, "Ack"),
            Signals::Stop => write!(f, "Stop"),
            Signals::Ping => write!(f, "Ping"),
        }
    }
}

impl Signals {
    pub fn as_cmdline(&self) -> Vec<u8> {
        format!("{}\n", self).as_bytes().to_vec()
    }
    pub fn as_string(&self) -> String {
        format!("{}", self).to_string()
    }
}
