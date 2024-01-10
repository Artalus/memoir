use interprocess::local_socket as ipc;

pub fn socket_name() -> String {
    use ipc::NameTypeSupport::*;
    match ipc::NameTypeSupport::query() {
        OnlyPaths => String::from("/tmp/memoirrs.sock"),
        OnlyNamespaced | Both => String::from("@memoirrs.sock"),
    }
}

pub enum Signal {
    Ack,
    Stop,
    Ping,
    Save,
}

impl std::fmt::Display for Signal {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Signal::Ack => write!(f, "Ack"),
            Signal::Stop => write!(f, "Stop"),
            Signal::Ping => write!(f, "Ping"),
            Signal::Save => write!(f, "Save"),
        }
    }
}

impl Signal {
    pub fn as_cmdline(&self) -> Vec<u8> {
        format!("{}\n", self).as_bytes().to_vec()
    }
    pub fn as_string(&self) -> String {
        format!("{}", self).to_string()
    }
}
