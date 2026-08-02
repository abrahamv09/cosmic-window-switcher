// SPDX-License-Identifier: GPL-3.0-only

use std::{
    env, fs,
    path::{Path, PathBuf},
    process::{Command, ExitStatus},
    sync::atomic::{AtomicUsize, Ordering},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use zbus::blocking::connection;

const BUS_NAME: &str = "io.github.abrahamv09.CosmicWindowSwitcher";
const OBJECT_PATH: &str = "/io/github/abrahamv09/CosmicWindowSwitcher";
const SCENARIO_ENV: &str = "COSMIC_WINDOW_SWITCHER_BUS_SCENARIO";
const SERVICE_PROCESS_ENV: &str = "COSMIC_WINDOW_SWITCHER_TEST_SERVICE_PROCESS";
const SERVICE_LOG_ENV: &str = "COSMIC_WINDOW_SWITCHER_TEST_SERVICE_LOG";
const FALLBACK_LOG_ENV: &str = "COSMIC_WINDOW_SWITCHER_TEST_FALLBACK_LOG";

fn binary() -> &'static str {
    env!("CARGO_BIN_EXE_cosmic-window-switcher")
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn unique_test_directory(name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock follows the Unix epoch")
        .as_nanos();
    env::temp_dir().join(format!(
        "cosmic-window-switcher-{name}-{}-{nonce}",
        std::process::id()
    ))
}

fn run_isolated_test(scenario: &str, extra_environment: &[(&str, &Path)]) -> ExitStatus {
    let current_test = env::current_exe().expect("resolve the integration test executable");
    let lifecycle_directory = unique_test_directory("invocation-lifecycle");
    fs::create_dir_all(&lifecycle_directory).expect("create isolated lifecycle directory");
    let test_name = thread::current()
        .name()
        .expect("the Rust test harness names test threads")
        .to_owned();
    let mut command = Command::new("dbus-run-session");
    command
        .arg("--")
        .arg(&current_test)
        .arg("--exact")
        .arg(test_name)
        .arg("--nocapture")
        .env(SCENARIO_ENV, scenario)
        .env("XDG_CURRENT_DESKTOP", "COSMIC")
        .env("XDG_SESSION_TYPE", "wayland")
        .env("XDG_CONFIG_HOME", lifecycle_directory.join("config"))
        .env("XDG_STATE_HOME", lifecycle_directory.join("state"));
    for (name, value) in extra_environment {
        command.env(name, value);
    }
    command.status().expect("run the isolated session-bus test")
}

#[test]
fn absent_service_falls_back_directly_in_the_requested_direction() {
    if env::var(SCENARIO_ENV).as_deref() != Ok("absent") {
        let directory = unique_test_directory("absent");
        fs::create_dir_all(&directory).expect("create isolated test directory");
        let fallback_log = directory.join("fallback.log");
        let status = run_isolated_test("absent", &[(FALLBACK_LOG_ENV, &fallback_log)]);
        assert!(status.success());
        assert_eq!(
            fs::read_to_string(fallback_log).expect("read stock fallback invocation"),
            "shift-alt-tab\n"
        );
        return;
    }

    let status = Command::new(binary())
        .args(["invoke", "previous"])
        .env(
            "COSMIC_WINDOW_SWITCHER_STOCK_LAUNCHER",
            fixture("record-stock-launcher.sh"),
        )
        .status()
        .expect("invoke the command against an isolated session bus");
    assert!(status.success());
}

#[test]
fn dbus_activation_recovers_the_service_and_delivers_forward_once() {
    if env::var(SCENARIO_ENV).as_deref() != Ok("activation") {
        let directory = unique_test_directory("activation");
        let service_directory = directory.join("dbus-1/services");
        fs::create_dir_all(&service_directory).expect("create isolated D-Bus service directory");
        let service_log = directory.join("service.log");
        let current_test = env::current_exe().expect("resolve the integration test executable");
        let service_file = format!(
            "[D-BUS Service]\nName={BUS_NAME}\nExec=/usr/bin/env {SERVICE_PROCESS_ENV}=1 {} --exact activated_service_process --nocapture\n",
            current_test.display()
        );
        fs::write(
            service_directory.join(format!("{BUS_NAME}.service")),
            service_file,
        )
        .expect("write isolated D-Bus activation metadata");

        let status = run_isolated_test(
            "activation",
            &[
                ("XDG_DATA_HOME", &directory),
                (SERVICE_LOG_ENV, &service_log),
            ],
        );
        assert!(status.success());
        assert_eq!(
            fs::read_to_string(service_log).expect("read delivered invocation"),
            "next\n"
        );
        return;
    }

    let status = Command::new(binary())
        .args(["invoke", "next"])
        .status()
        .expect("invoke the D-Bus-activated service");
    assert!(status.success());
}

struct RecordingService {
    log: PathBuf,
}

#[zbus::interface(name = "io.github.abrahamv09.CosmicWindowSwitcher1")]
impl RecordingService {
    fn invoke(&self, direction: &str) {
        fs::write(&self.log, format!("{direction}\n"))
            .expect("record the private invocation payload");
    }
}

#[test]
fn activated_service_process() {
    if env::var_os(SERVICE_PROCESS_ENV).is_none() {
        return;
    }

    let log = PathBuf::from(env::var_os(SERVICE_LOG_ENV).expect("service log path is configured"));
    let connection = connection::Builder::session()
        .expect("create activated service connection builder")
        .serve_at(OBJECT_PATH, RecordingService { log: log.clone() })
        .expect("serve the test interface")
        .name(BUS_NAME)
        .expect("request the activated service name")
        .build()
        .expect("build the activated service connection");
    let deadline = Instant::now() + Duration::from_secs(3);
    while !log.exists() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(10));
    }
    assert!(log.exists(), "the invocation was not delivered");
    drop(connection);
}

static UNRESPONSIVE_CALLS: AtomicUsize = AtomicUsize::new(0);

struct UnresponsiveService {
    calls: &'static AtomicUsize,
}

#[zbus::interface(name = "io.github.abrahamv09.CosmicWindowSwitcher1")]
impl UnresponsiveService {
    fn invoke(&self, direction: &str) {
        assert_eq!(direction, "next");
        self.calls.fetch_add(1, Ordering::SeqCst);
        thread::sleep(Duration::from_millis(400));
    }
}

#[test]
fn unresponsive_service_has_one_bounded_retry_then_falls_back() {
    if env::var(SCENARIO_ENV).as_deref() != Ok("unresponsive") {
        let directory = unique_test_directory("unresponsive");
        fs::create_dir_all(&directory).expect("create isolated test directory");
        let fallback_log = directory.join("fallback.log");
        let status = run_isolated_test("unresponsive", &[(FALLBACK_LOG_ENV, &fallback_log)]);
        assert!(status.success());
        assert_eq!(
            fs::read_to_string(fallback_log).expect("read stock fallback invocation"),
            "alt-tab\n"
        );
        return;
    }

    let connection = connection::Builder::session()
        .expect("create unresponsive service connection builder")
        .serve_at(
            OBJECT_PATH,
            UnresponsiveService {
                calls: &UNRESPONSIVE_CALLS,
            },
        )
        .expect("serve the unresponsive interface")
        .name(BUS_NAME)
        .expect("request the unresponsive service name")
        .build()
        .expect("build the unresponsive service connection");
    let started = Instant::now();
    let status = Command::new(binary())
        .args(["invoke", "next"])
        .env(
            "COSMIC_WINDOW_SWITCHER_STOCK_LAUNCHER",
            fixture("record-stock-launcher.sh"),
        )
        .status()
        .expect("invoke the unresponsive service");
    assert!(status.success());
    assert!(started.elapsed() < Duration::from_secs(2));
    assert_eq!(UNRESPONSIVE_CALLS.load(Ordering::SeqCst), 2);
    drop(connection);
}
