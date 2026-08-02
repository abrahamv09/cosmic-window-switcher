// SPDX-License-Identifier: GPL-3.0-only

use std::{
    env, fs,
    path::Path,
    process::Command,
    thread,
    time::{Duration, Instant},
};

use tempfile::TempDir;

const SHORTCUTS_PATH: &str = "cosmic/com.system76.CosmicSettings.Shortcuts/v1/system_actions";
const KEY_BINDINGS_PATH: &str = "cosmic/com.system76.CosmicSettings.Shortcuts/v1/custom";
const LIFECYCLE_PATH: &str = "cosmic/io.github.abrahamv09.CosmicWindowSwitcher/v1/integration";
const NEXT_COMMAND: &str = "/usr/bin/cosmic-window-switcher invoke next";
const PREVIOUS_COMMAND: &str = "/usr/bin/cosmic-window-switcher invoke previous";
const CONCURRENT_INVOKE_SCENARIO: &str = "COSMIC_WINDOW_SWITCHER_CONCURRENT_INVOKE_SCENARIO";

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
        )
        .env(
            "COSMIC_WINDOW_SWITCHER_TEST_SERVICE_STATE",
            sandbox.path().join("service.state"),
        );
    process
}

fn isolated_bus_command(sandbox: &TempDir, command: &str) -> Command {
    let mut process = Command::new("dbus-run-session");
    process
        .arg("--")
        .arg(binary())
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
        )
        .env(
            "COSMIC_WINDOW_SWITCHER_TEST_SERVICE_STATE",
            sandbox.path().join("service.state"),
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

fn seed_interrupted_enablement(sandbox: &TempDir) {
    seed_shortcuts(sandbox);
    let shortcuts = fs::read_to_string(shortcut_file(sandbox))
        .expect("read prior commands")
        .replace("prior-next", NEXT_COMMAND)
        .replace("prior-previous", PREVIOUS_COMMAND);
    fs::write(shortcut_file(sandbox), shortcuts).expect("simulate interrupted shortcut write");
    let state_path = lifecycle_file(sandbox);
    fs::create_dir_all(state_path.parent().expect("lifecycle state has a parent"))
        .expect("create lifecycle state directory");
    fs::write(
        state_path,
        "Enabling((next: Some(\"prior-next\"), previous: Some(\"prior-previous\")))\n",
    )
    .expect("simulate pending enablement journal");
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
        "--user enable cosmic-window-switcher.service\n--user start cosmic-window-switcher.service\n"
    );
    assert_eq!(
        fs::read_to_string(sandbox.path().join("service.state"))
            .expect("read enabled service state"),
        "active\n"
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
    assert_eq!(
        fs::read_to_string(sandbox.path().join("service.state"))
            .expect("read disabled service state"),
        "inactive\n"
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
fn upgrade_and_repeated_operations_preserve_manual_edits_after_enablement() {
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
    fs::write(sandbox.path().join("service.state"), "inactive\n")
        .expect("model the user unit stopping during package upgrade");

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
    assert_eq!(
        fs::read_to_string(sandbox.path().join("service.state"))
            .expect("read service state after upgrade re-enable"),
        "active\n"
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
fn concurrent_lifecycle_commands_serialize_shortcut_and_service_state() {
    let sandbox = TempDir::new().expect("create lifecycle sandbox");
    seed_shortcuts(&sandbox);
    let barrier = sandbox.path().join("enable-barrier");
    let mut enabling = lifecycle_command(&sandbox, "enable");
    enabling.env("COSMIC_WINDOW_SWITCHER_TEST_ENABLE_BARRIER", &barrier);
    let mut enabling = enabling.spawn().expect("start paused enablement");
    let deadline = Instant::now() + Duration::from_secs(3);
    while !barrier.with_extension("reached").exists() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(10));
    }
    assert!(
        barrier.with_extension("reached").exists(),
        "enablement never reached the service-control barrier"
    );

    let mut disabling = lifecycle_command(&sandbox, "disable")
        .spawn()
        .expect("start concurrent disablement");
    thread::sleep(Duration::from_millis(50));
    assert!(
        disabling
            .try_wait()
            .expect("inspect concurrent disablement")
            .is_none(),
        "disablement bypassed the lifecycle lock"
    );
    fs::write(barrier.with_extension("release"), "release\n").expect("release paused enablement");

    assert!(enabling.wait().expect("finish enablement").success());
    assert!(disabling.wait().expect("finish disablement").success());
    let restored = fs::read_to_string(shortcut_file(&sandbox)).expect("read serialized commands");
    assert!(restored.contains("prior-next"));
    assert!(restored.contains("prior-previous"));
    assert!(!restored.contains(NEXT_COMMAND));
    assert!(!restored.contains(PREVIOUS_COMMAND));
    assert!(
        fs::read_to_string(lifecycle_file(&sandbox))
            .expect("read serialized lifecycle state")
            .contains("Disabled")
    );
}

#[test]
fn concurrent_enable_and_invocation_cannot_deadlock_service_activation() {
    if env::var_os(CONCURRENT_INVOKE_SCENARIO).is_none() {
        let current_test = env::current_exe().expect("resolve lifecycle test executable");
        let status = Command::new("dbus-run-session")
            .arg("--")
            .arg(current_test)
            .args([
                "--exact",
                "concurrent_enable_and_invocation_cannot_deadlock_service_activation",
                "--nocapture",
            ])
            .env(CONCURRENT_INVOKE_SCENARIO, "1")
            .status()
            .expect("run concurrency scenario on an isolated session bus");
        assert!(status.success());
        return;
    }

    let sandbox = TempDir::new().expect("create lifecycle sandbox");
    seed_shortcuts(&sandbox);
    let barrier = sandbox.path().join("invoke-enable-barrier");
    let mut enabling = lifecycle_command(&sandbox, "enable");
    enabling.env("COSMIC_WINDOW_SWITCHER_TEST_ENABLE_BARRIER", &barrier);
    let mut enabling = enabling.spawn().expect("start paused enablement");
    let deadline = Instant::now() + Duration::from_secs(3);
    while !barrier.with_extension("reached").exists() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(10));
    }
    assert!(barrier.with_extension("reached").exists());

    let fallback_log = sandbox.path().join("concurrent-fallback.log");
    let mut invoking = lifecycle_command(&sandbox, "invoke");
    invoking
        .arg("next")
        .env(
            "COSMIC_WINDOW_SWITCHER_STOCK_LAUNCHER",
            fixture("record-stock-launcher.sh"),
        )
        .env("COSMIC_WINDOW_SWITCHER_TEST_FALLBACK_LOG", &fallback_log);
    let mut invoking = invoking.spawn().expect("start concurrent invocation");
    thread::sleep(Duration::from_millis(50));
    assert!(
        invoking
            .try_wait()
            .expect("inspect concurrent invocation")
            .is_none(),
        "invocation bypassed the lifecycle lock"
    );
    fs::write(barrier.with_extension("release"), "release\n").expect("release paused enablement");

    assert!(enabling.wait().expect("finish enablement").success());
    assert!(invoking.wait().expect("finish invocation").success());
    let configured = fs::read_to_string(shortcut_file(&sandbox)).expect("read enabled commands");
    assert!(configured.contains(NEXT_COMMAND));
    assert!(configured.contains(PREVIOUS_COMMAND));
    assert!(
        fs::read_to_string(lifecycle_file(&sandbox))
            .expect("read enabled lifecycle state")
            .contains("Enabled")
    );
    assert_eq!(
        fs::read_to_string(fallback_log).expect("read concurrent stock fallback"),
        "alt-tab\n"
    );
}

#[test]
fn interrupted_enablement_journal_recovers_before_the_next_invocation() {
    let sandbox = TempDir::new().expect("create lifecycle sandbox");
    seed_interrupted_enablement(&sandbox);

    let fallback_log = sandbox.path().join("fallback.log");
    let status = isolated_bus_command(&sandbox, "invoke")
        .arg("previous")
        .env(
            "COSMIC_WINDOW_SWITCHER_STOCK_LAUNCHER",
            fixture("record-stock-launcher.sh"),
        )
        .env("COSMIC_WINDOW_SWITCHER_TEST_FALLBACK_LOG", &fallback_log)
        .status()
        .expect("invoke after interrupted enablement");

    assert!(status.success());
    let restored = fs::read_to_string(shortcut_file(&sandbox)).expect("read recovered commands");
    assert!(restored.contains("prior-next"));
    assert!(restored.contains("prior-previous"));
    assert!(!restored.contains(NEXT_COMMAND));
    assert!(!restored.contains(PREVIOUS_COMMAND));
    assert_eq!(
        fs::read_to_string(fallback_log).expect("read stock fallback direction"),
        "shift-alt-tab\n"
    );
}

#[test]
fn service_start_recovers_an_interrupted_enablement_before_claiming_dbus() {
    let sandbox = TempDir::new().expect("create lifecycle sandbox");
    seed_interrupted_enablement(&sandbox);

    let status = isolated_bus_command(&sandbox, "service")
        .status()
        .expect("start service with pending journal");

    assert!(status.success());
    let restored = fs::read_to_string(shortcut_file(&sandbox)).expect("read recovered commands");
    assert!(restored.contains("prior-next"));
    assert!(restored.contains("prior-previous"));
    assert!(!restored.contains(NEXT_COMMAND));
    assert!(!restored.contains(PREVIOUS_COMMAND));
    assert!(
        fs::read_to_string(lifecycle_file(&sandbox))
            .expect("read recovered lifecycle state")
            .contains("Disabled")
    );
    assert!(
        fs::read_to_string(sandbox.path().join("systemctl.log"))
            .expect("read recovery service control")
            .contains("--user disable cosmic-window-switcher.service")
    );
}

#[test]
fn uninstall_cleanup_restores_owned_values_without_a_cosmic_session() {
    let sandbox = TempDir::new().expect("create lifecycle sandbox");
    seed_shortcuts(&sandbox);
    assert!(
        lifecycle_command(&sandbox, "enable")
            .status()
            .expect("enable integration")
            .success()
    );
    let shortcut_path = shortcut_file(&sandbox);
    let manually_edited = fs::read_to_string(&shortcut_path)
        .expect("read enabled commands")
        .replace(NEXT_COMMAND, "manual-next");
    fs::write(&shortcut_path, manually_edited).expect("edit command before uninstall");

    let status = lifecycle_command(&sandbox, "disable")
        .arg("--uninstall")
        .env("XDG_CURRENT_DESKTOP", "")
        .env("XDG_SESSION_TYPE", "")
        .status()
        .expect("run uninstall cleanup outside COSMIC");

    assert!(status.success());
    let restored = fs::read_to_string(shortcut_path).expect("read uninstalled commands");
    assert!(restored.contains("manual-next"));
    assert!(restored.contains("prior-previous"));
    assert!(!restored.contains(NEXT_COMMAND));
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
    assert!(
        fs::read_to_string(lifecycle_file(&sandbox))
            .expect("read retained recovery journal")
            .contains("Enabling")
    );

    assert!(
        lifecycle_command(&sandbox, "disable")
            .status()
            .expect("retry rollback")
            .success()
    );
    assert!(
        fs::read_to_string(lifecycle_file(&sandbox))
            .expect("read completed recovery state")
            .contains("Disabled")
    );
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
        let output = isolated_bus_command(&sandbox, command)
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

#[test]
fn unsupported_status_still_reports_existing_shortcut_ownership() {
    let sandbox = TempDir::new().expect("create lifecycle sandbox");
    seed_interrupted_enablement(&sandbox);

    let output = lifecycle_command(&sandbox, "status")
        .env("XDG_CURRENT_DESKTOP", "GNOME")
        .env("XDG_SESSION_TYPE", "x11")
        .output()
        .expect("inspect unsupported-session health");

    assert!(output.status.success());
    let report = String::from_utf8(output.stdout).expect("diagnostics are UTF-8");
    assert!(report.contains("session: unsupported"));
    assert!(report.contains("shortcut_ownership: owned"));
}
