// SPDX-License-Identifier: GPL-3.0-only

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use zbus::{
    blocking::{Connection, Proxy, connection},
    fdo::{RequestNameFlags, RequestNameReply},
    zvariant::Type,
};

use cosmic_window_switcher::{
    InvocationDirection, MruHistoryAccuracy, ServiceDiagnostics, WindowId,
    WorkspaceEligibilityDiagnostics,
};

use super::{BUS_NAME, INTERFACE_NAME, OBJECT_PATH, PendingInvocations, SharedService};

pub(super) fn serve(
    service: SharedService,
    pending_invocations: PendingInvocations,
) -> Result<Connection> {
    let connection = connection::Builder::session()
        .context("connect to the user-session D-Bus")?
        .serve_at(
            OBJECT_PATH,
            ServiceInterface {
                service,
                pending_invocations,
            },
        )
        .context("register the Switcher Service D-Bus interface")?
        .build()
        .context("start the Switcher Service D-Bus connection")?;
    let reply = connection
        .request_name_with_flags(BUS_NAME, RequestNameFlags::DoNotQueue.into())
        .context("request the single Switcher Service D-Bus name")?;
    if reply != RequestNameReply::PrimaryOwner && reply != RequestNameReply::AlreadyOwner {
        anyhow::bail!("another Switcher Service already owns the user-session D-Bus name");
    }
    Ok(connection)
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
    workspace_eligibility: String,
    toplevel_info_version: u32,
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
            workspace_eligibility: match diagnostics.workspace_eligibility {
                WorkspaceEligibilityDiagnostics::AwaitingSnapshot => "awaiting-snapshot",
                WorkspaceEligibilityDiagnostics::Ready => "ready",
                WorkspaceEligibilityDiagnostics::MissingToplevelMembership { .. } => {
                    "missing-toplevel-membership"
                }
            }
            .to_owned(),
            toplevel_info_version: match diagnostics.workspace_eligibility {
                WorkspaceEligibilityDiagnostics::MissingToplevelMembership {
                    advertised_version,
                } => advertised_version,
                WorkspaceEligibilityDiagnostics::AwaitingSnapshot
                | WorkspaceEligibilityDiagnostics::Ready => 0,
            },
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
            workspace_eligibility: match diagnostics.workspace_eligibility.as_str() {
                "ready" => WorkspaceEligibilityDiagnostics::Ready,
                "missing-toplevel-membership" => {
                    WorkspaceEligibilityDiagnostics::MissingToplevelMembership {
                        advertised_version: diagnostics.toplevel_info_version,
                    }
                }
                _ => WorkspaceEligibilityDiagnostics::AwaitingSnapshot,
            },
        }
    }
}

struct ServiceInterface {
    service: SharedService,
    pending_invocations: PendingInvocations,
}

#[zbus::interface(name = "io.github.abrahamv09.CosmicWindowSwitcher1")]
impl ServiceInterface {
    fn status(&self) -> DbusDiagnostics {
        self.service
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .diagnostics()
            .into()
    }

    fn invoke(&self, direction: &str) -> zbus::fdo::Result<()> {
        let direction = match direction {
            "next" => InvocationDirection::Next,
            "previous" => InvocationDirection::Previous,
            _ => {
                return Err(zbus::fdo::Error::InvalidArgs(
                    "direction must be next or previous".to_owned(),
                ));
            }
        };
        self.pending_invocations.push(direction);
        Ok(())
    }
}
