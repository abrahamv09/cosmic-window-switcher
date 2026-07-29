// SPDX-License-Identifier: GPL-3.0-only

use anyhow::Result;
use clap::{Parser, Subcommand};

mod probe;

#[derive(Debug, Parser)]
#[command(
    name = "cosmic-window-switcher",
    version,
    about = "A native Window switcher for the COSMIC desktop"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Run the interactive two-Window COSMIC integration probe.
    Probe {
        /// Include Window titles in temporary probe output.
        #[arg(long)]
        include_titles: bool,
    },
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::Probe { include_titles } => probe::run(include_titles),
    }
}
