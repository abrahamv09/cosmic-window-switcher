// SPDX-License-Identifier: GPL-3.0-only

use anyhow::Result;
use clap::{Parser, Subcommand};
use cosmic_window_switcher::{InvocationDirection, Locale};

mod cosmic_session;
mod probe;
mod service;
mod settings;
mod shm_capture;
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
    /// Configure visual and performance preferences.
    Settings,
    /// Request a forward or reverse Window switch from the resident service.
    Invoke {
        #[command(subcommand)]
        direction: InvokeDirection,
    },
    /// Run the interactive two-Window COSMIC integration probe.
    Probe {
        /// Include Window titles in temporary probe output.
        #[arg(long)]
        include_titles: bool,
        /// Repeat frames on compositor damage and report the Live Thumbnail contract.
        #[arg(long)]
        live_thumbnails: bool,
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

#[derive(Clone, Copy, Debug, Subcommand)]
enum InvokeDirection {
    /// Select the next Window in MRU Order.
    Next,
    /// Select the previous Window in reverse MRU Order.
    Previous,
}

impl From<InvokeDirection> for InvocationDirection {
    fn from(direction: InvokeDirection) -> Self {
        match direction {
            InvokeDirection::Next => Self::Next,
            InvokeDirection::Previous => Self::Previous,
        }
    }
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::Service => service::run(),
        Command::Status => {
            println!("{}", service::status()?.localized(Locale::detect()));
            Ok(())
        }
        Command::Settings => settings::run(),
        Command::Invoke { direction } => service::invoke(direction.into()),
        Command::Probe {
            include_titles,
            live_thumbnails,
        } => probe::run(include_titles, live_thumbnails),
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
