use std::vec::Vec;

pub struct Process {
    name: String,
    memory_mb: u64,
    commandline: String,
}

impl std::fmt::Display for Process {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "<{} | {} | {}>", self.name, self.memory_mb, self.commandline)
    }
}

pub struct CurrentProcesses {
    timestamp: u128,
    processes: Vec<Process>,
}
impl std::fmt::Display for CurrentProcesses {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{} - >", self.timestamp)?;
        for p in &self.processes {
            write!(f, " <{} | {} | {}>", p.name, p.memory_mb, p.commandline)?
        }
        Ok(())
    }
}

pub fn list_processes() -> CurrentProcesses {
    use std::time::{SystemTime, UNIX_EPOCH};
    CurrentProcesses {
        timestamp: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_millis(),
        processes: vec![
            Process {
                name: "process1".to_string(),
                memory_mb: 1024,
                commandline: "commandline1".to_string(),
            },
        ],
    }
}
