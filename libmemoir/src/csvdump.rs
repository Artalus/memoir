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
    let iterator = CsvIterator::new(writer, time_sec, history.iter().enumerate());
    for line in iterator {
        match line {
            Ok(()) => {}
            Err(e) => return Err(e),
        }
    }
    Ok(())
}

pub struct CsvIterator<W: std::io::Write, I> {
    input: I,
    pub writer: csv::Writer<W>,
    started: bool,
    write_since: u128,
}

impl<W: std::io::Write, I> CsvIterator<W, I> {
    pub fn new(writer: csv::Writer<W>, time_sec: Option<usize>, input: I) -> CsvIterator<W, I> {
        use std::time::{SystemTime, UNIX_EPOCH};
        let since = match time_sec {
            Some(t) => {
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .expect("Time went before Unix epoch!")
                    .as_millis()
                    - (t as u128 * 1_000)
            }
            None => 0,
        };

        CsvIterator {
            input: input,
            started: false,
            writer: writer,
            write_since: since,
        }
    }
}

pub fn iterator_on_buffer(
    input: std::collections::vec_deque::Iter<'_, crate::process::CurrentProcesses>,
    time_sec: Option<usize>,
) -> CsvIterator<
    std::io::Cursor<Vec<u8>>,
    std::iter::Enumerate<std::collections::vec_deque::Iter<'_, CurrentProcesses>>,
> {
    let buffer = Vec::new();
    let cursor = std::io::Cursor::new(buffer);
    // let mut writer1 = std::io::BufWriter::new(&mut buffer);
    let writer = csv::WriterBuilder::new()
        .delimiter(b'\t')
        .from_writer(cursor);
    let enumerated = input.enumerate();
    let iterator = crate::csvdump::CsvIterator::new(writer, time_sec, enumerated);
    iterator
}

impl<'a, W: std::io::Write, I: Iterator<Item = (usize, &'a CurrentProcesses)>> Iterator
    for CsvIterator<W, I>
{
    type Item = anyhow::Result<()>;

    fn next(&mut self) -> Option<anyhow::Result<()>> {
        if !self.started {
            self.started = true;
            return Some(
                self.writer
                    .write_record([
                        "Iteration",
                        "Timestamp",
                        "PID",
                        "Name",
                        "Memory MB",
                        "Command line",
                    ])
                    .context("Could not dump header in iterator"),
            );
        }
        let x = self.input.next();
        if x.is_none() {
            return None;
        }
        let (iteration, processes) = x.unwrap();
        if processes.timestamp < self.write_since {
            return Some(Ok(()));
        }
        for entry in &processes.entries {
            match self.writer.write_record(&[
                (iteration + 1).to_string(),
                processes.timestamp.to_string(),
                entry.process.pid.to_string(),
                entry.process.name.to_string(),
                entry.memory_mb.to_string(),
                escape_cmdline(&entry.process.commandline),
            ]) {
                Ok(()) => {}
                Err(e) => {
                    return Some(
                        Err(anyhow::Error::from(e)).context("Failed to write record into writer"),
                    )
                }
            }
        }

        Some(Ok(()))
    }
}

fn escape_cmdline(cmdline: &str) -> String {
    cmdline.replace('\t', "\\t").replace('\n', "\\n")
}
