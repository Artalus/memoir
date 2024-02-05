use std::collections::VecDeque;
use std::path::PathBuf;

use anyhow::Context;

use crate::process::CurrentProcesses;

pub fn save_to_file(
    history: &VecDeque<CurrentProcesses>,
    destination: &PathBuf,
    time_sec: Option<usize>,
) -> anyhow::Result<()> {
    let writer = csv::WriterBuilder::new()
        .delimiter(b'\t')
        .from_path(destination)
        .context(format!("Could not create CSV writer for {:?}", destination))?;
    save_to(history, writer, time_sec)
}

pub fn save_to_stream<W: std::io::Write>(
    history: &VecDeque<CurrentProcesses>,
    writer: W,
    time_sec: Option<usize>,
) -> anyhow::Result<()> {
    let writer = csv::WriterBuilder::new()
        .delimiter(b'\t')
        .from_writer(writer);
    save_to(history, writer, time_sec)
}

fn save_to<W: std::io::Write>(
    history: &VecDeque<CurrentProcesses>,
    mut writer: csv::Writer<W>,
    time_sec: Option<usize>,
) -> anyhow::Result<()> {
    use std::time::{SystemTime, UNIX_EPOCH};
    let since = match time_sec {
        Some(t) => {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .context("Time went backwards! TODO: support timey-wimey stuff in memoir")?
                .as_millis()
                - (t as u128 * 1_000)
        }
        None => 0,
    };
    writer.write_record([
        "Iteration",
        "Timestamp",
        "PID",
        "Name",
        "Memory MB",
        "Command line",
    ])?;
    for (iteration, processes) in history.iter().enumerate() {
        if processes.timestamp < since {
            continue;
        }
        for entry in &processes.entries {
            writer.write_record(&[
                (iteration + 1).to_string(),
                processes.timestamp.to_string(),
                entry.process.pid.to_string(),
                entry.process.name.to_string(),
                entry.memory_mb.to_string(),
                escape_cmdline(&entry.process.commandline),
            ])?;
        }
    }
    Ok(())
}

fn escape_cmdline(cmdline: &str) -> String {
    cmdline.replace('\t', "\\t").replace('\n', "\\n")
}
