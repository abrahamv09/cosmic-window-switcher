// SPDX-License-Identifier: GPL-3.0-only

use anyhow::Result;
use clap::{CommandFactory, FromArgMatches, Parser, Subcommand};
use cosmic_window_switcher::{InvocationDirection, Locale, StringKey};

mod cosmic_session;
mod probe;
mod service;
mod settings;
mod shm_capture;

#[derive(Debug, Parser)]
#[command(name = "cosmic-window-switcher", version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Service,
    Status,
    Settings,
    Invoke {
        #[command(subcommand)]
        direction: InvokeDirection,
    },
    Probe {
        #[arg(long)]
        include_titles: bool,
        #[arg(long)]
        live_thumbnails: bool,
    },
}

#[derive(Clone, Copy, Debug, Subcommand)]
enum InvokeDirection {
    Next,
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
    let matches = match localized_command(locale).try_get_matches() {
        Ok(matches) => matches,
        Err(error)
            if matches!(
                error.kind(),
                clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion
            ) =>
        {
            error.exit()
        }
        Err(_) => {
            let mut command = localized_command(locale);
            eprintln!("{}", localized_cli_error(locale, &mut command));
            std::process::exit(2);
        }
    };
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
    }
}

fn localized_cli_error(locale: Locale, command: &mut clap::Command) -> String {
    format!(
        "{}\n\n{}",
        locale.text(StringKey::CliInvalidArguments),
        command.render_long_help()
    )
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

    #[test]
    fn spanish_parse_errors_do_not_fall_back_to_clap_english() {
        let mut command = localized_command(Locale::Spanish);
        let rendered = localized_cli_error(Locale::Spanish, &mut command);

        assert!(rendered.starts_with("Los argumentos de la línea de comandos"));
        assert!(rendered.contains("Uso:"));
        assert!(!rendered.contains("unexpected argument"));
        assert!(!rendered.contains("For more information"));
    }
}
