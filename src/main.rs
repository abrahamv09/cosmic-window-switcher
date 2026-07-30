// SPDX-License-Identifier: GPL-3.0-only

use anyhow::Result;
use clap::{CommandFactory, FromArgMatches, Parser, Subcommand};
use cosmic_window_switcher::{InvocationDirection, Locale, StringKey};

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
    let locale = Locale::detect();
    let matches = localized_command(locale).get_matches();
    let cli = Cli::from_arg_matches(&matches).expect("Clap generated and parsed the same command");
    match cli.command {
        Command::Service => service::run(),
        Command::Status => {
            println!("{}", service::status()?.localized(locale));
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

fn localized_command(locale: Locale) -> clap::Command {
    localized_command_frame(Cli::command(), locale)
        .about(locale.text(StringKey::CliAbout))
        .disable_version_flag(true)
        .arg(
            clap::Arg::new("localized-version")
                .short('V')
                .long("version")
                .action(clap::ArgAction::Version)
                .help(locale.text(StringKey::CliVersionOption)),
        )
        .mut_subcommand("service", |command| {
            localized_command_frame(command, locale).about(locale.text(StringKey::CliService))
        })
        .mut_subcommand("status", |command| {
            localized_command_frame(command, locale).about(locale.text(StringKey::CliStatus))
        })
        .mut_subcommand("settings", |command| {
            localized_command_frame(command, locale).about(locale.text(StringKey::CliSettings))
        })
        .mut_subcommand("invoke", |command| {
            localized_command_frame(command, locale)
                .about(locale.text(StringKey::CliInvoke))
                .mut_subcommand("next", |next| {
                    localized_command_frame(next, locale).about(locale.text(StringKey::CliNext))
                })
                .mut_subcommand("previous", |previous| {
                    localized_command_frame(previous, locale)
                        .about(locale.text(StringKey::CliPrevious))
                })
        })
        .mut_subcommand("probe", |command| {
            localized_command_frame(command, locale)
                .about(locale.text(StringKey::CliProbe))
                .mut_arg("include_titles", |argument| {
                    argument.help(locale.text(StringKey::CliIncludeTitles))
                })
                .mut_arg("live_thumbnails", |argument| {
                    argument.help(locale.text(StringKey::CliLiveThumbnails))
                })
        })
        .mut_subcommand("probe-workspace-move", |command| {
            localized_command_frame(command, locale)
                .about(locale.text(StringKey::CliProbeWorkspaceMove))
                .mut_arg("window", |argument| {
                    argument.help(locale.text(StringKey::CliWindow))
                })
                .mut_arg("workspace", |argument| {
                    argument.help(locale.text(StringKey::CliWorkspace))
                })
                .mut_arg("output", |argument| {
                    argument.help(locale.text(StringKey::CliOutput))
                })
        })
}

fn localized_command_frame(command: clap::Command, locale: Locale) -> clap::Command {
    let template = format!(
        "{{about-with-newline}}\n{} {{usage}}\n\n{}\n{{subcommands}}\n\n{}\n{{options}}",
        locale.text(StringKey::CliUsageHeading),
        locale.text(StringKey::CliCommandsHeading),
        locale.text(StringKey::CliOptionsHeading),
    );
    command
        .disable_help_subcommand(true)
        .disable_help_flag(true)
        .help_template(template)
        .arg(
            clap::Arg::new("localized-help")
                .short('h')
                .long("help")
                .action(clap::ArgAction::Help)
                .help(locale.text(StringKey::CliHelpOption)),
        )
}

#[cfg(test)]
mod cli_tests {
    use super::*;

    #[test]
    fn spanish_help_localizes_commands_and_arguments() {
        let help = localized_command(Locale::Spanish)
            .render_long_help()
            .to_string();

        assert!(help.contains("Un selector de ventanas nativo"));
        assert!(help.contains("Configurar las preferencias visuales"));
        assert!(!help.contains("Configure visual and performance preferences"));
    }
}
