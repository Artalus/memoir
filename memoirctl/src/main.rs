extern crate memoir;

use clap::{Parser, Subcommand};

/// Memoir is a small tool to monitor current RAM consumption on per-process basis
#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// get current RAM info, print and exit
    Once,
    /// start RAM monitoring
    Run {
        #[arg(short, long)]
        without_checks: bool,
        /// how many entries / seconds of history to keep
        #[arg(long, default_value_t = 3600)]
        #[arg(value_parser = parsetime::parse_time)]
        keep_history: usize,
    },
    /// start as a detached daemon
    Detach {
        /// how many entries / seconds of history to keep
        #[arg(long, default_value_t = 3600)]
        #[arg(value_parser = parsetime::parse_time)]
        keep_history: usize,
    },
    /// stop a running daemon
    Stop,
    /// check if daemon is running
    Status,
    /// save collected RAM report to a file
    Save {
        /// path to save to
        path: String,
    },
}

pub fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    match &args.command {
        Commands::Once => memoir::control::do_once(),
        Commands::Detach { keep_history } => memoir::control::do_detach(keep_history.to_owned()),
        Commands::Run {
            without_checks,
            keep_history,
        } => memoir::control::do_run(!without_checks, keep_history.to_owned()),
        Commands::Stop => memoir::control::do_stop(),
        Commands::Status => memoir::control::do_status(),
        Commands::Save { path } => memoir::control::do_save(path),
    }
}

mod parsetime;
