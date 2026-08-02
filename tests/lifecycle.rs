// SPDX-License-Identifier: GPL-3.0-only

use std::{fs, path::Path, process::Command};

use tempfile::TempDir;

const SHORTCUTS_PATH: &str = "cosmic/com.system76.CosmicSettings.Shortcuts/v1/system_actions";
const KEY_BINDINGS_PATH: &str = "cosmic/com.system76.CosmicSettings.Shortcuts/v1/custom";
const LIFECYCLE_PATH: &str = "cosmic/io.github.abrahamv09.CosmicWindowSwitcher/v1/integration";
const NEXT_COMMAND: &str = "/usr/bin/cosmic-window-switcher invoke next";
const PREVIOUS_COMMAND: &str = "/usr/bin/cosmic-window-switcher invoke previous";

fn binary() -> &'static str {
    env!("CARGO_BIN_EXE_cosmic-window-switcher")
}

fn fixture(name: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn lifecycle_command(sandbox: &TempDir, command: &str) -> Command {
    let mut process = Command::new(binary());
    process
        .arg(command)
        .env("XDG_CURRENT_DESKTOP", "COSMIC")
        .env("XDG_SESSION_TYPE", "wayland")
        .env("XDG_CONFIG_HOME", sandbox.path().join("config"))
        .env("XDG_STATE_HOME", sandbox.path().join("state"))
        .env(
            "COSMIC_WINDOW_SWITCHER_SYSTEMCTL",
            fixture("record-systemctl.sh"),
        )
        .env(
            "COSMIC_WINDOW_SWITCHER_TEST_SYSTEMCTL_LOG",
            sandbox.path().join("systemctl.log"),
        );
    process
}

fn shortcut_file(sandbox: &TempDir) -> std::path::PathBuf {
    sandbox.path().join("config").join(SHORTCUTS_PATH)
}

fn lifecycle_file(sandbox: &TempDir) -> std::path::PathBuf {
    sandbox.path().join("state").join(LIFECYCLE_PATH)
}

fn key_bindings_file(sandbox: &TempDir) -> std::path::PathBuf {
    sandbox.path().join("config").join(KEY_BINDINGS_PATH)
}

fn seed_shortcuts(sandbox: &TempDir) {
    let path = shortcut_file(sandbox);
    fs::create_dir_all(path.parent().expect("shortcut file has a parent"))
        .expect("create shortcut configuration directory");
    fs::write(
        path,
        r#"{
    Launcher: "custom-launcher",
    WindowSwitcher: "prior-next",
    WindowSwitcherPrevious: "prior-previous",
}
"#,
    )
    .expect("seed user semantic commands");
    fs::write(
        key_bindings_file(sandbox),
        "{ /* user-owned key bindings */ }\n",
    )
    .expect("seed user key bindings");
}

#[test]
fn enable_and_disable_restore_owned_values_without_touching_key_bindings() {
    let sandbox = TempDir::new().expect("create lifecycle sandbox");
    seed_shortcuts(&sandbox);

    let enabled = lifecycle_command(&sandbox, "enable")
        .status()
        .expect("enable integration");

    assert!(enabled.success());
    let configured = fs::read_to_string(shortcut_file(&sandbox)).expect("read enabled commands");
    assert!(configured.contains(NEXT_COMMAND));
    assert!(configured.contains(PREVIOUS_COMMAND));
    assert!(configured.contains("custom-launcher"));
    assert_eq!(
        fs::read_to_string(key_bindings_file(&sandbox)).expect("read user key bindings"),
        "{ /* user-owned key bindings */ }\n"
    );
    assert!(lifecycle_file(&sandbox).is_file());
    assert_eq!(
        fs::read_to_string(sandbox.path().join("systemctl.log")).expect("read service command"),
        "--user enable --now cosmic-window-switcher.service\n"
    );

    let disabled = lifecycle_command(&sandbox, "disable")
        .status()
        .expect("disable integration");

    assert!(disabled.success());
    let restored = fs::read_to_string(shortcut_file(&sandbox)).expect("read restored commands");
    assert!(restored.contains("prior-next"));
    assert!(restored.contains("prior-previous"));
    assert!(restored.contains("custom-launcher"));
    assert!(!restored.contains(NEXT_COMMAND));
    assert!(!restored.contains(PREVIOUS_COMMAND));
    assert!(
        fs::read_to_string(sandbox.path().join("systemctl.log"))
            .expect("read service commands")
            .ends_with("--user disable --now cosmic-window-switcher.service\n")
    );
}

#[test]
fn disable_removes_app_commands_that_had_no_prior_user_values() {
    let sandbox = TempDir::new().expect("create lifecycle sandbox");

    assert!(
        lifecycle_command(&sandbox, "enable")
            .status()
            .expect("enable fresh integration")
            .success()
    );
    assert!(
        lifecycle_command(&sandbox, "disable")
            .status()
            .expect("disable fresh integration")
            .success()
    );

    let restored = fs::read_to_string(shortcut_file(&sandbox)).expect("read restored commands");
    assert!(!restored.contains("WindowSwitcher"));
    assert!(!restored.contains(NEXT_COMMAND));
    assert!(!restored.contains(PREVIOUS_COMMAND));
}

#[test]
fn repeated_operations_preserve_manual_edits_after_enablement() {
    let sandbox = TempDir::new().expect("create lifecycle sandbox");
    seed_shortcuts(&sandbox);
    assert!(
        lifecycle_command(&sandbox, "enable")
            .status()
            .expect("enable integration")
            .success()
    );

    let path = shortcut_file(&sandbox);
    let manually_edited = fs::read_to_string(&path)
        .expect("read enabled commands")
        .replace(NEXT_COMMAND, "manual-next");
    fs::write(&path, manually_edited).expect("edit the forward semantic command");

    assert!(
        lifecycle_command(&sandbox, "enable")
            .status()
            .expect("repeat enablement")
            .success()
    );
    assert!(
        fs::read_to_string(&path)
            .expect("read commands after repeated enablement")
            .contains("manual-next")
    );

    assert!(
        lifecycle_command(&sandbox, "disable")
            .status()
            .expect("disable integration")
            .success()
    );
    assert!(
        lifecycle_command(&sandbox, "disable")
            .status()
            .expect("repeat disablement")
            .success()
    );
    let restored = fs::read_to_string(path).expect("read disabled commands");
    assert!(restored.contains("manual-next"));
    assert!(restored.contains("prior-previous"));
    assert!(!restored.contains(PREVIOUS_COMMAND));
}

#[test]
fn failed_service_enablement_rolls_back_both_semantic_commands() {
    let sandbox = TempDir::new().expect("create lifecycle sandbox");
    seed_shortcuts(&sandbox);

    let status = lifecycle_command(&sandbox, "enable")
        .env("COSMIC_WINDOW_SWITCHER_SYSTEMCTL", "/bin/false")
        .status()
        .expect("attempt enablement");

    assert!(!status.success());
    let restored = fs::read_to_string(shortcut_file(&sandbox)).expect("read rolled-back commands");
    assert!(restored.contains("prior-next"));
    assert!(restored.contains("prior-previous"));
    assert!(!restored.contains(NEXT_COMMAND));
    assert!(!restored.contains(PREVIOUS_COMMAND));
}

#[test]
fn unsupported_sessions_are_rejected_before_any_integration_change() {
    let sandbox = TempDir::new().expect("create lifecycle sandbox");

    let status = lifecycle_command(&sandbox, "enable")
        .env("XDG_CURRENT_DESKTOP", "GNOME:ubuntu")
        .env("XDG_SESSION_TYPE", "x11")
        .status()
        .expect("reject unsupported session");

    assert!(!status.success());
    assert!(!shortcut_file(&sandbox).exists());
    assert!(!lifecycle_file(&sandbox).exists());
    assert!(!sandbox.path().join("systemctl.log").exists());
}

#[test]
fn unsupported_session_invocation_neither_activates_service_nor_launches_fallback() {
    let sandbox = TempDir::new().expect("create lifecycle sandbox");
    let fallback_log = sandbox.path().join("fallback.log");

    let status = Command::new(binary())
        .args(["invoke", "next"])
        .env("XDG_CURRENT_DESKTOP", "GNOME:ubuntu")
        .env("XDG_SESSION_TYPE", "x11")
        .env(
            "COSMIC_WINDOW_SWITCHER_STOCK_LAUNCHER",
            fixture("record-stock-launcher.sh"),
        )
        .env("COSMIC_WINDOW_SWITCHER_TEST_FALLBACK_LOG", &fallback_log)
        .status()
        .expect("reject unsupported invocation");

    assert!(!status.success());
    assert!(!fallback_log.exists());
}

#[test]
fn status_and_doctor_report_privacy_safe_lifecycle_health() {
    let sandbox = TempDir::new().expect("create lifecycle sandbox");

    for command in ["status", "doctor"] {
        let output = lifecycle_command(&sandbox, command)
            .output()
            .expect("inspect lifecycle health");
        assert!(output.status.success());
        let report = String::from_utf8(output.stdout).expect("diagnostics are UTF-8");
        assert!(report.contains("service: stopped"));
        assert!(report.contains("capabilities: unavailable"));
        assert!(report.contains("capture_backend: unavailable"));
        assert!(report.contains("mru_history: unavailable"));
        assert!(report.contains("shortcut_ownership: not-owned"));
        assert!(!report.contains("Window title"));
        assert!(!report.contains("pixel"));
    }
}
