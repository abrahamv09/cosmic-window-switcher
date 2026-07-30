// SPDX-License-Identifier: GPL-3.0-only

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use zbus::{
    blocking::{Connection, Proxy, connection},
    fdo::{RequestNameFlags, RequestNameReply},
    zvariant::Type,
};

use cosmic_window_switcher::{
    CaptureBackend, CaptureBackendSelection, DmaBufCompatibility, DmaBufFallbackReason,
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
    match proxy.call::<_, _, DbusDiagnosticsV2>("Status2", &()) {
        Ok(diagnostics) => Ok(diagnostics.into()),
        Err(versioned_error) => {
            let diagnostics: DbusDiagnostics = proxy.call("Status", &()).with_context(|| {
                format!("request Switcher Service status after Status2 failed: {versioned_error}")
            })?;
            Ok(diagnostics.into())
        }
    }
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

#[derive(Deserialize, Serialize, Type)]
struct DbusDiagnosticsV2 {
    diagnostics: DbusDiagnostics,
    capture_backend: String,
    capture_backend_fallback: String,
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

impl From<ServiceDiagnostics> for DbusDiagnosticsV2 {
    fn from(diagnostics: ServiceDiagnostics) -> Self {
        Self {
            capture_backend: diagnostics
                .capture_backend
                .backend()
                .diagnostic_name()
                .to_owned(),
            capture_backend_fallback: diagnostics
                .capture_backend
                .fallback_reason()
                .map_or_else(String::new, |reason| reason.diagnostic_name().to_owned()),
            diagnostics: diagnostics.into(),
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
            capture_backend: CaptureBackendSelection::default(),
        }
    }
}

impl From<DbusDiagnosticsV2> for ServiceDiagnostics {
    fn from(diagnostics: DbusDiagnosticsV2) -> Self {
        let fallback_reason = match diagnostics.capture_backend_fallback.as_str() {
            "incompatible-device" => DmaBufFallbackReason::IncompatibleDevice,
            "unsupported-format" => DmaBufFallbackReason::UnsupportedFormat,
            "unsupported-modifier" => DmaBufFallbackReason::UnsupportedModifier,
            "allocation-failed" => DmaBufFallbackReason::AllocationFailed,
            "synchronization-unavailable" => DmaBufFallbackReason::SynchronizationUnavailable,
            "release-unavailable" => DmaBufFallbackReason::ReleaseUnavailable,
            _ => DmaBufFallbackReason::ImportFailed,
        };
        let mut service_diagnostics = ServiceDiagnostics::from(diagnostics.diagnostics);
        service_diagnostics.capture_backend =
            if diagnostics.capture_backend == CaptureBackend::DmaBuf.diagnostic_name() {
                DmaBufCompatibility::complete().select_backend()
            } else {
                CaptureBackendSelection::shared_memory(fallback_reason)
            };
        service_diagnostics
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

    fn status2(&self) -> DbusDiagnosticsV2 {
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
                capture_backend: CaptureBackendSelection::default(),
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
            capture_backend: CaptureBackendSelection::default(),
        };
        assert_eq!(
            ServiceDiagnostics::from(DbusDiagnostics::from(visible.clone())),
            visible
        );
    }

    #[test]
    fn versioned_dbus_diagnostics_preserve_capture_backend_selection() {
        let selections = [
            DmaBufCompatibility::complete().select_backend(),
            CaptureBackendSelection::shared_memory(DmaBufFallbackReason::IncompatibleDevice),
            CaptureBackendSelection::shared_memory(DmaBufFallbackReason::UnsupportedFormat),
            CaptureBackendSelection::shared_memory(DmaBufFallbackReason::UnsupportedModifier),
            CaptureBackendSelection::shared_memory(DmaBufFallbackReason::AllocationFailed),
            CaptureBackendSelection::shared_memory(
                DmaBufFallbackReason::SynchronizationUnavailable,
            ),
            CaptureBackendSelection::shared_memory(DmaBufFallbackReason::ImportFailed),
            CaptureBackendSelection::shared_memory(DmaBufFallbackReason::ReleaseUnavailable),
        ];

        for capture_backend in selections {
            let diagnostics = ServiceDiagnostics {
                mru_history: MruHistoryAccuracy::Accurate,
                mru_order: vec![WindowId::from("opaque")],
                window_scope: WindowScope::AllWorkspaces,
                workspace_eligibility: WorkspaceEligibilityState::Ready,
                capture_backend,
            };

            assert_eq!(
                ServiceDiagnostics::from(DbusDiagnosticsV2::from(diagnostics.clone())),
                diagnostics
            );
        }
    }
}
