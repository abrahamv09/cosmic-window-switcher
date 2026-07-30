// SPDX-License-Identifier: GPL-3.0-only

use anyhow::Result;
use clap::{Parser, Subcommand};

mod cosmic_session;
mod probe;
mod service;
mod workspace_move_probe;

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
    /// Run the resident Switcher Service.
    Service,
    /// Report the resident service's current MRU Order.
    Status,
    /// Run the interactive two-Window COSMIC integration probe.
    Probe {
        /// Include Window titles in temporary probe output.
        #[arg(long)]
        include_titles: bool,
    },
    /// Inventory workspace capabilities or verify one advertised workspace move.
    ProbeWorkspaceMove {
        /// Opaque Window id printed by inventory mode.
        #[arg(long, requires = "workspace")]
        window: Option<String>,
        /// Target workspace selector printed by inventory mode.
        #[arg(long, requires = "window")]
        workspace: Option<String>,
        /// Target output name; needed only when it cannot be selected unambiguously.
        #[arg(long, requires = "window")]
        output: Option<String>,
    },
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::Service => service::run(),
        Command::Status => {
            println!("{}", service::status()?);
            Ok(())
        }
        Command::Probe { include_titles } => probe::run(include_titles),
        Command::ProbeWorkspaceMove {
            window,
            workspace,
            output,
        } => workspace_move_probe::run(workspace_move_probe::Mode::from_options(
            window.as_deref(),
            workspace.as_deref(),
            output.as_deref(),
        )?),
    }
}
