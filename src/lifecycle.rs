// SPDX-License-Identifier: GPL-3.0-only

use std::{collections::BTreeMap, path::PathBuf, process::Command};

use anyhow::{Context, Result};
use cosmic_config::{Config, ConfigGet, ConfigSet};
use cosmic_settings_config::shortcuts::{
    self, SystemActions,
    action::System::{self, WindowSwitcher, WindowSwitcherPrevious},
};
use cosmic_window_switcher::{APPLICATION_ID, Locale, StringKey};
use serde::{Deserialize, Serialize};

const STATE_VERSION: u64 = 1;
const STATE_KEY: &str = "integration";
const SERVICE_UNIT: &str = "cosmic-window-switcher.service";
const SYSTEMCTL: &str = "/usr/bin/systemctl";
const SYSTEMCTL_OVERRIDE: &str = "COSMIC_WINDOW_SWITCHER_SYSTEMCTL";
const NEXT_COMMAND: &str = "/usr/bin/cosmic-window-switcher invoke next";
const PREVIOUS_COMMAND: &str = "/usr/bin/cosmic-window-switcher invoke previous";

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
struct PreviousCommands {
    next: Option<String>,
    previous: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
enum IntegrationState {
    #[default]
    Disabled,
    Enabling(PreviousCommands),
    Enabled(PreviousCommands),
}

#[derive(Clone, Copy, Debug)]
enum ShortcutOwnership {
    Owned,
    Partial,
    NotOwned,
    Unavailable,
}

pub(super) fn enable() -> Result<()> {
    crate::cosmic_session::verify("integration lifecycle")?;
    let state_store = state_store()?;
    let state = recover_interrupted_enable(&state_store, load_state(&state_store)?)?;

    if matches!(state, IntegrationState::Enabled(_)) {
        return control_service("enable");
    }

    let shortcut_context = shortcut_context()?;
    let mut system_actions = local_system_actions(&shortcut_context)?;
    let previous = PreviousCommands {
        next: system_actions.get(&WindowSwitcher).cloned(),
        previous: system_actions.get(&WindowSwitcherPrevious).cloned(),
    };
    save_state(&state_store, &IntegrationState::Enabling(previous.clone()))?;

    system_actions.insert(WindowSwitcher, NEXT_COMMAND.to_owned());
    system_actions.insert(WindowSwitcherPrevious, PREVIOUS_COMMAND.to_owned());
    if let Err(shortcut_error) = write_system_actions(&shortcut_context, &system_actions) {
        save_state(&state_store, &IntegrationState::Disabled).with_context(|| {
            format!("clear pending enablement after shortcut write failed: {shortcut_error}")
        })?;
        return Err(shortcut_error);
    }

    if let Err(service_error) = control_service("enable") {
        let _service_rollback = control_service("disable");
        rollback_enable(&state_store, &shortcut_context, &previous).with_context(|| {
            format!("roll back semantic commands after service enablement failed: {service_error}")
        })?;
        return Err(service_error);
    }

    if let Err(state_error) = save_state(&state_store, &IntegrationState::Enabled(previous.clone()))
    {
        let _service_rollback = control_service("disable");
        rollback_enable(&state_store, &shortcut_context, &previous).with_context(|| {
            format!(
                "roll back integration after its enabled state could not be saved: {state_error}"
            )
        })?;
        return Err(state_error);
    }

    Ok(())
}

pub(super) fn disable() -> Result<()> {
    crate::cosmic_session::verify("integration lifecycle")?;
    let state_store = state_store()?;
    let state = recover_interrupted_enable(&state_store, load_state(&state_store)?)?;

    let previous = match state {
        IntegrationState::Enabled(previous) => Some(previous),
        IntegrationState::Disabled | IntegrationState::Enabling(_) => None,
    };
    control_service("disable")?;

    if let Some(previous) = previous {
        let shortcut_context = shortcut_context()?;
        restore_owned_commands(&shortcut_context, &previous)?;
    }
    save_state(&state_store, &IntegrationState::Disabled)
}

pub(super) fn diagnostics(locale: Locale) -> Result<String> {
    let compatible = crate::cosmic_session::is_compatible();
    let ownership = if compatible {
        shortcut_ownership().unwrap_or(ShortcutOwnership::Unavailable)
    } else {
        ShortcutOwnership::NotOwned
    };
    let mut lines = vec![localized_status(
        locale,
        StringKey::Session,
        if compatible {
            StringKey::Compatible
        } else {
            StringKey::Unsupported
        },
    )];

    if compatible && service::running() {
        lines.push(localized_status(
            locale,
            StringKey::Capabilities,
            StringKey::Ready,
        ));
        lines.push(localized_status(
            locale,
            StringKey::ShortcutOwnership,
            ownership.key(),
        ));
        lines.push(service::status()?.localized(locale));
    } else {
        lines.extend([
            localized_status(locale, StringKey::Service, StringKey::Stopped),
            localized_status(locale, StringKey::Capabilities, StringKey::Unavailable),
            localized_status(locale, StringKey::CaptureBackend, StringKey::Unavailable),
            localized_status(locale, StringKey::MruHistory, StringKey::Unavailable),
            localized_status(locale, StringKey::ShortcutOwnership, ownership.key()),
        ]);
    }

    Ok(lines.join("\n"))
}

fn recover_interrupted_enable(
    state_store: &Config,
    state: IntegrationState,
) -> Result<IntegrationState> {
    let IntegrationState::Enabling(previous) = state else {
        return Ok(state);
    };
    let shortcut_context = shortcut_context()?;
    restore_owned_commands(&shortcut_context, &previous)?;
    let _service_rollback = control_service("disable");
    save_state(state_store, &IntegrationState::Disabled)?;
    Ok(IntegrationState::Disabled)
}

fn rollback_enable(
    state_store: &Config,
    shortcut_context: &Config,
    previous: &PreviousCommands,
) -> Result<()> {
    restore_owned_commands(shortcut_context, previous)?;
    save_state(state_store, &IntegrationState::Disabled)
}

fn restore_owned_commands(context: &Config, previous: &PreviousCommands) -> Result<()> {
    let mut system_actions = local_system_actions(context)?;
    let next_restored = restore_if_owned(
        &mut system_actions,
        WindowSwitcher,
        NEXT_COMMAND,
        previous.next.as_deref(),
    );
    let previous_restored = restore_if_owned(
        &mut system_actions,
        WindowSwitcherPrevious,
        PREVIOUS_COMMAND,
        previous.previous.as_deref(),
    );
    if next_restored || previous_restored {
        write_system_actions(context, &system_actions)?;
    }
    Ok(())
}

fn restore_if_owned(
    system_actions: &mut BTreeMap<System, String>,
    action: System,
    owned_command: &str,
    previous: Option<&str>,
) -> bool {
    if system_actions.get(&action).map(String::as_str) != Some(owned_command) {
        return false;
    }
    if let Some(previous) = previous {
        system_actions.insert(action, previous.to_owned());
    } else {
        system_actions.remove(&action);
    }
    true
}

fn shortcut_ownership() -> Result<ShortcutOwnership> {
    let context = shortcut_context()?;
    let system_actions = local_system_actions(&context)?;
    let next_owned = system_actions.get(&WindowSwitcher).map(String::as_str) == Some(NEXT_COMMAND);
    let previous_owned = system_actions
        .get(&WindowSwitcherPrevious)
        .map(String::as_str)
        == Some(PREVIOUS_COMMAND);
    Ok(match (next_owned, previous_owned) {
        (true, true) => ShortcutOwnership::Owned,
        (true, false) | (false, true) => ShortcutOwnership::Partial,
        (false, false) => ShortcutOwnership::NotOwned,
    })
}

fn local_system_actions(context: &Config) -> Result<SystemActions> {
    match context.get_local("system_actions") {
        Ok(actions) => Ok(actions),
        Err(cosmic_config::Error::NotFound) => Ok(SystemActions::new()),
        Err(error) => Err(error).context("read user semantic commands"),
    }
}

fn shortcut_context() -> Result<Config> {
    Config::new(shortcuts::ID, 1).context("open COSMIC Shortcut Policy")
}

fn write_system_actions(context: &Config, system_actions: &SystemActions) -> Result<()> {
    context
        .set("system_actions", system_actions)
        .context("transactionally write user semantic commands")
}

fn state_store() -> Result<Config> {
    Config::new_state(APPLICATION_ID, STATE_VERSION).context("open integration lifecycle state")
}

fn load_state(store: &Config) -> Result<IntegrationState> {
    match store.get_local(STATE_KEY) {
        Ok(state) => Ok(state),
        Err(cosmic_config::Error::NotFound) => Ok(IntegrationState::Disabled),
        Err(error) => Err(error).context("read integration lifecycle state"),
    }
}

fn save_state(store: &Config, state: &IntegrationState) -> Result<()> {
    store
        .set(STATE_KEY, state)
        .context("atomically save integration lifecycle state")
}

fn control_service(operation: &str) -> Result<()> {
    let executable = std::env::var_os(SYSTEMCTL_OVERRIDE)
        .map_or_else(|| PathBuf::from(SYSTEMCTL), PathBuf::from);
    let status = Command::new(&executable)
        .args(["--user", operation, "--now", SERVICE_UNIT])
        .status()
        .with_context(|| format!("run {} for {SERVICE_UNIT}", executable.display()))?;
    if !status.success() {
        anyhow::bail!("{operation} {SERVICE_UNIT} failed with status {status}");
    }
    Ok(())
}

fn localized_status(locale: Locale, label: StringKey, value: StringKey) -> String {
    format!("{}: {}", locale.text(label), locale.text(value))
}

impl ShortcutOwnership {
    const fn key(self) -> StringKey {
        match self {
            Self::Owned => StringKey::Owned,
            Self::Partial => StringKey::PartiallyOwned,
            Self::NotOwned => StringKey::NotOwned,
            Self::Unavailable => StringKey::Unavailable,
        }
    }
}

mod service {
    pub(super) use crate::service::{running, status};
}
