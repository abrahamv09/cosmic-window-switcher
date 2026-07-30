// SPDX-License-Identifier: GPL-3.0-only

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use zbus::{
    blocking::{Connection, Proxy, connection},
    zvariant::Type,
};

use cosmic_window_switcher::{MruHistoryAccuracy, ServiceDiagnostics, WindowId};

use super::SharedService;

const BUS_NAME: &str = "io.github.abrahamv09.CosmicWindowSwitcher";
const OBJECT_PATH: &str = "/io/github/abrahamv09/CosmicWindowSwitcher";
const INTERFACE_NAME: &str = "io.github.abrahamv09.CosmicWindowSwitcher1";

pub(super) fn serve(service: SharedService) -> Result<Connection> {
    connection::Builder::session()
        .context("connect to the user-session D-Bus")?
        .serve_at(OBJECT_PATH, DiagnosticsInterface { service })
        .context("register the Switcher Service D-Bus interface")?
        .name(BUS_NAME)
        .context("request the single Switcher Service D-Bus name")?
        .build()
        .context("start the Switcher Service D-Bus connection")
}

pub(super) fn status() -> Result<ServiceDiagnostics> {
    let connection =
        Connection::session().context("connect to the user-session D-Bus for status")?;
    let proxy = Proxy::new(&connection, BUS_NAME, OBJECT_PATH, INTERFACE_NAME)
        .context("create the Switcher Service status proxy")?;
    let diagnostics: DbusDiagnostics = proxy
        .call("Status", &())
        .context("request Switcher Service status")?;

    Ok(diagnostics.into())
}

#[derive(Deserialize, Serialize, Type)]
struct DbusDiagnostics {
    mru_warm_up: bool,
    mru_order: Vec<String>,
}

impl From<ServiceDiagnostics> for DbusDiagnostics {
    fn from(diagnostics: ServiceDiagnostics) -> Self {
        Self {
            mru_warm_up: diagnostics.mru_history == MruHistoryAccuracy::WarmUp,
            mru_order: diagnostics
                .mru_order
                .into_iter()
                .map(|id| id.as_str().to_owned())
                .collect(),
        }
    }
}

impl From<DbusDiagnostics> for ServiceDiagnostics {
    fn from(diagnostics: DbusDiagnostics) -> Self {
        Self {
            mru_history: if diagnostics.mru_warm_up {
                MruHistoryAccuracy::WarmUp
            } else {
                MruHistoryAccuracy::Accurate
            },
            mru_order: diagnostics
                .mru_order
                .into_iter()
                .map(WindowId::from)
                .collect(),
        }
    }
}

struct DiagnosticsInterface {
    service: SharedService,
}

#[zbus::interface(name = "io.github.abrahamv09.CosmicWindowSwitcher1")]
impl DiagnosticsInterface {
    fn status(&self) -> DbusDiagnostics {
        self.service
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .diagnostics()
            .into()
    }
}
