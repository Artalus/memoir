use std::vec::Vec;

pub struct Process {
    pid: i32,
    name: String,
    memory_mb: u64,
    commandline: String,
}

impl std::fmt::Display for Process {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "<{} | {} | {} | {}>", self.pid, self.name, self.memory_mb, self.commandline)
    }
}

pub struct CurrentProcesses {
    timestamp: u128,
    processes: Vec<Process>,
}
impl std::fmt::Display for CurrentProcesses {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        writeln!(f, "--- {} ---", self.timestamp)?;
        for p in &self.processes {
            writeln!(f, "    {}", p)?
        }
        Ok(())
    }
}

pub fn list_processes() -> CurrentProcesses {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_millis();
    let page_size: u64 = procfs::page_size();
    let mut processes: Vec<Process> = Vec::with_capacity(100);

    // panic if cannot list processes at all - this is unexpected
    for prc in procfs::process::all_processes().unwrap() {
        // but silently ignore everything we cannot access - processes may die
        if let Err(_) = prc {
            continue;
        }
        let prc = prc.unwrap();
        let stat = prc.stat();
        if let Err(_) = stat {
            continue;
        }
        let stat = stat.unwrap();
        if stat.rss == 0 {
            continue;
        }
        let executable = match prc.exe() {
            Ok(e) => match e.into_os_string().into_string() {
                Ok(e) => e,
                Err(_) => String::from("?"),
            },
            Err(_) => String::from("?"),
        };
        let cmd = match prc.cmdline() {
            Ok(c) => c.join(" "),
            Err(_) => String::from("?"),
        };
        processes.push(
            Process {
                pid: prc.pid,
                name: executable,
                memory_mb: stat.rss * page_size / 1_000_000,
                commandline: cmd,
            }
        )
    }
    CurrentProcesses {
        timestamp: now,
        processes: processes
    }
}
