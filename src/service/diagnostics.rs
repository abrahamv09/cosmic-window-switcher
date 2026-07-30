// SPDX-License-Identifier: GPL-3.0-only

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use zbus::{
    blocking::{Connection, Proxy, connection},
    fdo::{RequestNameFlags, RequestNameReply},
    zvariant::Type,
};

use cosmic_window_switcher::{
    InvocationDirection, MruHistoryAccuracy, ServiceDiagnostics, WindowId, WindowScope,
    WorkspaceEligibilityState,
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
    window_scope: String,
    workspace_eligibility: String,
    advertised_version: u32,
    required_version: u32,
}

impl From<ServiceDiagnostics> for DbusDiagnostics {
    fn from(diagnostics: ServiceDiagnostics) -> Self {
        let (workspace_eligibility, advertised_version, required_version) =
            match diagnostics.workspace_eligibility {
                WorkspaceEligibilityState::AwaitingSnapshot => ("awaiting-snapshot", 0, 0),
                WorkspaceEligibilityState::Ready => ("ready", 0, 0),
                WorkspaceEligibilityState::MissingToplevelInfo {
                    advertised_version,
                    required_version,
                } => (
                    "missing-toplevel-info",
                    advertised_version.unwrap_or(0),
                    required_version,
                ),
                WorkspaceEligibilityState::MissingWorkspaceProtocol {
                    advertised_version,
                    required_version,
                } => (
                    "missing-workspace-protocol",
                    advertised_version.unwrap_or(0),
                    required_version,
                ),
                WorkspaceEligibilityState::MissingWorkspaceSnapshot { advertised_version } => {
                    ("missing-workspace-snapshot", advertised_version, 1)
                }
                WorkspaceEligibilityState::MissingToplevelMembership { advertised_version } => {
                    ("missing-toplevel-membership", advertised_version, 3)
                }
            };
        Self {
            mru_warm_up: diagnostics.mru_history == MruHistoryAccuracy::WarmUp,
            mru_order: diagnostics
                .mru_order
                .into_iter()
                .map(|id| id.as_str().to_owned())
                .collect(),
            window_scope: match diagnostics.window_scope {
                WindowScope::AllWorkspaces => "all-workspaces",
                WindowScope::VisibleWorkspaces => "visible-workspaces",
            }
            .to_owned(),
            workspace_eligibility: workspace_eligibility.to_owned(),
            advertised_version,
            required_version,
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
            window_scope: match diagnostics.window_scope.as_str() {
                "visible-workspaces" => WindowScope::VisibleWorkspaces,
                _ => WindowScope::AllWorkspaces,
            },
            workspace_eligibility: match diagnostics.workspace_eligibility.as_str() {
                "ready" => WorkspaceEligibilityState::Ready,
                "missing-toplevel-info" => WorkspaceEligibilityState::MissingToplevelInfo {
                    advertised_version: (diagnostics.advertised_version != 0)
                        .then_some(diagnostics.advertised_version),
                    required_version: diagnostics.required_version,
                },
                "missing-workspace-protocol" => {
                    WorkspaceEligibilityState::MissingWorkspaceProtocol {
                        advertised_version: (diagnostics.advertised_version != 0)
                            .then_some(diagnostics.advertised_version),
                        required_version: diagnostics.required_version,
                    }
                }
                "missing-workspace-snapshot" => {
                    WorkspaceEligibilityState::MissingWorkspaceSnapshot {
                        advertised_version: diagnostics.advertised_version,
                    }
                }
                "missing-toplevel-membership" => {
                    WorkspaceEligibilityState::MissingToplevelMembership {
                        advertised_version: diagnostics.advertised_version,
                    }
                }
                _ => WorkspaceEligibilityState::AwaitingSnapshot,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dbus_diagnostics_preserve_each_workspace_eligibility_state() {
        let states = [
            WorkspaceEligibilityState::AwaitingSnapshot,
            WorkspaceEligibilityState::Ready,
            WorkspaceEligibilityState::MissingToplevelInfo {
                advertised_version: None,
                required_version: 3,
            },
            WorkspaceEligibilityState::MissingToplevelInfo {
                advertised_version: Some(2),
                required_version: 3,
            },
            WorkspaceEligibilityState::MissingWorkspaceProtocol {
                advertised_version: None,
                required_version: 1,
            },
            WorkspaceEligibilityState::MissingWorkspaceSnapshot {
                advertised_version: 1,
            },
            WorkspaceEligibilityState::MissingToplevelMembership {
                advertised_version: 3,
            },
        ];

        for state in states {
            let diagnostics = ServiceDiagnostics {
                mru_history: MruHistoryAccuracy::WarmUp,
                mru_order: vec![WindowId::from("opaque")],
                window_scope: WindowScope::AllWorkspaces,
                workspace_eligibility: state,
            };

            assert_eq!(
                ServiceDiagnostics::from(DbusDiagnostics::from(diagnostics.clone())),
                diagnostics
            );
        }

        let visible = ServiceDiagnostics {
            mru_history: MruHistoryAccuracy::Accurate,
            mru_order: Vec::new(),
            window_scope: WindowScope::VisibleWorkspaces,
            workspace_eligibility: WorkspaceEligibilityState::Ready,
        };
        assert_eq!(
            ServiceDiagnostics::from(DbusDiagnostics::from(visible.clone())),
            visible
        );
    }
}
