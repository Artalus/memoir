use std::{collections::HashSet, sync::Arc, vec::Vec};

#[derive(Eq, Hash, PartialEq)]
pub struct Process {
    pub pid: i32,
    pub name: String,
    pub commandline: String,
}

pub struct HistoryEntry {
    pub process: Arc<Process>,
    pub memory_mb: u64,
}

impl std::fmt::Display for HistoryEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "<{} | {} | {} | {}>",
            self.process.pid, self.process.name, self.memory_mb, self.process.commandline
        )
    }
}

pub struct CurrentProcesses {
    pub timestamp: u128,
    pub entries: Vec<HistoryEntry>,
}
impl std::fmt::Display for CurrentProcesses {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        writeln!(f, "--- {} ---", self.timestamp)?;
        for p in &self.entries {
            writeln!(f, "    {}", p)?
        }
        Ok(())
    }
}

// List all processes that are currently running. Since most of pids and names will be repeated
// between iterations, use a cache to avoid having tens of megabytes of same strings in memory.
pub fn list_processes(process_cache: &mut HashSet<Arc<Process>>) -> CurrentProcesses {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_millis();
    let page_size: u64 = procfs::page_size();
    let mut entries: Vec<HistoryEntry> = Vec::with_capacity(100);

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
        // `Arc<T>` can be compared with `T`, so we can get ref-counted process from
        // cache by its "raw" structure.
        let potential_entry = Process { pid: prc.pid, name: executable, commandline: cmd };
        let cached = match process_cache.get(&potential_entry) {
            Some(c) => c.clone(),
            None => Arc::from(potential_entry),
        };
        let _ins = process_cache.insert(cached.clone());

        entries.push(HistoryEntry { process: cached, memory_mb: stat.rss * page_size / 1_000_000 })
    }
    CurrentProcesses {
        timestamp: now,
        entries: entries,
    }
}
