use std::collections::VecDeque;
use std::path::PathBuf;

use anyhow::Context;
use csv;

use crate::process::CurrentProcesses;

pub fn save_to_csv(history: &VecDeque<CurrentProcesses>, destination: &PathBuf) -> anyhow::Result<()> {
    let mut writer = csv::WriterBuilder::new()
        .delimiter(b'\t')
        .from_path(destination)
        .context(format!("Could not create CSV writer for {:?}", destination))
    ?;
    writer.write_record(&["Iteration", "Timestamp", "PID", "Name", "Memory MB", "Command line"])?;
    for (iteration, entry) in history.iter().enumerate() {
        for process in &entry.processes {
            writer.write_record(&[
                (iteration + 1).to_string(),
                entry.timestamp.to_string(),
                process.pid.to_string(),
                process.name.to_string(),
                process.memory_mb.to_string(),
                escape_cmdline(&process.commandline)
            ])?;
        }
    }
    Ok(())
}

fn escape_cmdline(cmdline: &str) -> String {
    cmdline.replace("\t", "\\t")
}
