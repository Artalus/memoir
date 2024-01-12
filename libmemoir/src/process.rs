use std::{collections::HashSet, sync::Arc, vec::Vec};

#[derive(Eq, Hash, PartialEq)]
pub struct Process {
    pub pid: u32,
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
    CurrentProcesses {
        timestamp: now,
        entries: platform_specific::platform_list_processes(process_cache),
    }
}

#[cfg(target_os = "linux")]
mod platform_specific {
    use std::{collections::HashSet, sync::Arc};
    use super::*;

    pub fn platform_list_processes(process_cache: &mut HashSet<Arc<Process>>) -> Vec<HistoryEntry> {
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
            let potential_entry = Process { pid: prc.pid as u32, name: executable, commandline: cmd };
            let cached = match process_cache.get(&potential_entry) {
                Some(c) => c.clone(),
                None => Arc::from(potential_entry),
            };
            let _ins = process_cache.insert(cached.clone());

            entries.push(HistoryEntry { process: cached, memory_mb: stat.rss * page_size / 1_000_000 })
        }
        entries
    }
}

#[cfg(target_os = "windows")]
mod platform_specific {
    #![allow(non_camel_case_types)]
    #![allow(non_snake_case)]
    use std::{collections::HashSet, sync::Arc};
    use super::*;
    use serde::Deserialize;

    // needs to be named exactly like the entity in WMI
    #[derive(Deserialize, Debug)]
    struct Win32_Process {
        ProcessId: u32,
        Name: Option<String>,
        WorkingSetSize: u64,
        CommandLine: Option<String>,
    }

    pub fn platform_list_processes(process_cache: &mut HashSet<Arc<Process>>) -> Vec<HistoryEntry> {
        let mut entries: Vec<HistoryEntry> = Vec::with_capacity(100);
        {
            let com_con = wmi::COMLibrary::new().expect("Could not acquire COM library");
            let wmi_con = wmi::WMIConnection::new(com_con.into()).expect("Could not establish WMI connection");
            // TODO: Win32_Process.WorkingSetSize is not exactly what we need... Better join with
            // Win32_PerfRawData_PerfProc_Process on WP.ProcessId == WPRDPPP.IDProcess, and get
            // WorkingSetPrivate from there.
            let result: Vec<Win32_Process> = wmi_con.query().expect("Could not query WMI for processes");
            for r in result {
                // `Arc<T>` can be compared with `T`, so we can get ref-counted process from
                // cache by its "raw" structure.
                let potential_entry = Process {
                    pid: r.ProcessId,
                    name: r.Name.unwrap_or("?".to_string()),
                    commandline: r.CommandLine.unwrap_or("?".to_string()),
                };
                let cached = match process_cache.get(&potential_entry) {
                    Some(c) => c.clone(),
                    None => Arc::from(potential_entry),
                };
                let _ins = process_cache.insert(cached.clone());

                entries.push(HistoryEntry {
                    process: cached,
                    memory_mb: r.WorkingSetSize / 1_000_000,
                });
            };
        }
        return entries
    }
}
