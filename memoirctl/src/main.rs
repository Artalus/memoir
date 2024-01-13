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
    },
    /// start as a detached daemon
    Detach,
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
        Commands::Once => {
            memoir::control::do_once();
            Ok(())
        }
        Commands::Detach => memoir::control::do_detach(),
        Commands::Run { without_checks } => memoir::control::do_run(!without_checks),
        Commands::Stop => memoir::control::do_stop(),
        Commands::Status => memoir::control::do_status(),
        Commands::Save { path } => memoir::control::do_save(path),
    }
}
