// SPDX-License-Identifier: GPL-3.0-only

use std::{path::PathBuf, process::Command, time::Duration};

use anyhow::{Context, Result};
use cosmic_window_switcher::InvocationDirection;
use zbus::{
    blocking::{Connection, Proxy, connection},
    names::WellKnownName,
};

use super::{BUS_NAME, INTERFACE_NAME, OBJECT_PATH};

const METHOD_TIMEOUT: Duration = Duration::from_millis(250);
const STOCK_LAUNCHER: &str = "/usr/bin/cosmic-launcher";
const STOCK_LAUNCHER_OVERRIDE: &str = "COSMIC_WINDOW_SWITCHER_STOCK_LAUNCHER";

pub(super) fn invoke(direction: InvocationDirection) -> Result<()> {
    if let Ok(connection) = invocation_connection() {
        if request(&connection, direction).is_ok() {
            return Ok(());
        }

        if let Ok(recovered) = invocation_connection() {
            if let Ok(proxy) = zbus::blocking::fdo::DBusProxy::new(&recovered) {
                let _recovery = proxy.start_service_by_name(
                    WellKnownName::try_from(BUS_NAME)
                        .expect("the stable application id is a valid D-Bus name"),
                    0,
                );
            }
            if request(&recovered, direction).is_ok() {
                return Ok(());
            }
        }
    }

    launch_stock_switcher(direction)
}

fn invocation_connection() -> zbus::Result<Connection> {
    connection::Builder::session()?
        .method_timeout(METHOD_TIMEOUT)
        .build()
}

fn request(connection: &Connection, direction: InvocationDirection) -> zbus::Result<()> {
    let proxy = Proxy::new(connection, BUS_NAME, OBJECT_PATH, INTERFACE_NAME)?;
    proxy.call("Invoke", &direction_name(direction))
}

pub(super) fn launch_stock_switcher(direction: InvocationDirection) -> Result<()> {
    let launcher = std::env::var_os(STOCK_LAUNCHER_OVERRIDE)
        .map_or_else(|| PathBuf::from(STOCK_LAUNCHER), PathBuf::from);
    let command = match direction {
        InvocationDirection::Next => "alt-tab",
        InvocationDirection::Previous => "shift-alt-tab",
    };
    let status = Command::new(&launcher)
        .arg(command)
        .status()
        .with_context(|| format!("launch stock COSMIC switcher using {}", launcher.display()))?;
    if !status.success() {
        anyhow::bail!("stock COSMIC switcher exited unsuccessfully with status {status}");
    }
    Ok(())
}

const fn direction_name(direction: InvocationDirection) -> &'static str {
    match direction {
        InvocationDirection::Next => "next",
        InvocationDirection::Previous => "previous",
    }
}
