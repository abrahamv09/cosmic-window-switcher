// SPDX-License-Identifier: GPL-3.0-only

use cosmic_window_switcher::{
    CaptureBackend, CaptureBackendSelection, DmaBufFallbackReason, Locale, MruHistoryAccuracy,
    ServiceDiagnostics, SwitcherService, WindowEvent, WindowId, WindowScope,
    WorkspaceEligibilityState,
};

fn window(id: &str) -> WindowId {
    WindowId::from(id)
}

#[test]
fn status_reports_the_active_capture_backend_and_privacy_safe_fallback_reason() {
    let mut service = SwitcherService::new();
    service.set_capture_backend(CaptureBackendSelection::shared_memory(
        DmaBufFallbackReason::ImportFailed,
    ));
    service.observe(WindowEvent::Discovered(window("opaque-window")));

    let diagnostics = service.diagnostics();
    let status = diagnostics.to_string();

    assert_eq!(
        diagnostics.capture_backend.backend(),
        CaptureBackend::SharedMemory
    );
    assert_eq!(
        diagnostics.capture_backend.fallback_reason(),
        Some(DmaBufFallbackReason::ImportFailed)
    );
    assert!(status.contains("capture_backend: shared-memory"));
    assert!(status.contains("capture_backend_fallback: DMA-BUF renderer import failed"));
    assert!(!status.contains("Window title"));
    assert!(!status.contains("pixel"));
}

#[test]
fn observed_focus_sequence_produces_current_first_mru_order() {
    let mut service = SwitcherService::new();
    service.observe(WindowEvent::Discovered(window("alpha")));
    service.observe(WindowEvent::Discovered(window("beta")));
    service.observe(WindowEvent::Discovered(window("gamma")));

    service.observe(WindowEvent::Activated(window("alpha")));
    service.observe(WindowEvent::Activated(window("beta")));
    service.observe(WindowEvent::Activated(window("gamma")));

    assert_eq!(
        service.diagnostics(),
        ServiceDiagnostics {
            mru_history: MruHistoryAccuracy::Accurate,
            mru_order: vec![window("gamma"), window("beta"), window("alpha")],
            window_scope: WindowScope::AllWorkspaces,
            workspace_eligibility: WorkspaceEligibilityState::AwaitingSnapshot,
            capture_backend: CaptureBackendSelection::default(),
        }
    );
}

#[test]
fn duplicate_activation_does_not_disturb_recency() {
    let mut service = SwitcherService::new();
    service.observe(WindowEvent::Discovered(window("alpha")));
    service.observe(WindowEvent::Discovered(window("beta")));
    service.observe(WindowEvent::Activated(window("alpha")));
    service.observe(WindowEvent::Activated(window("beta")));

    service.observe(WindowEvent::Activated(window("beta")));

    assert_eq!(
        service.diagnostics().mru_order,
        vec![window("beta"), window("alpha")]
    );
}

#[test]
fn closed_windows_disappear_without_reordering_survivors() {
    let mut service = SwitcherService::new();
    for id in ["alpha", "beta", "gamma"] {
        service.observe(WindowEvent::Discovered(window(id)));
        service.observe(WindowEvent::Activated(window(id)));
    }

    service.observe(WindowEvent::Closed(window("beta")));

    assert_eq!(
        service.diagnostics(),
        ServiceDiagnostics {
            mru_history: MruHistoryAccuracy::Accurate,
            mru_order: vec![window("gamma"), window("alpha")],
            window_scope: WindowScope::AllWorkspaces,
            workspace_eligibility: WorkspaceEligibilityState::AwaitingSnapshot,
            capture_backend: CaptureBackendSelection::default(),
        }
    );
}

#[test]
fn restart_reports_warm_up_with_current_first_and_stable_discovery_order() {
    let mut restarted_service = SwitcherService::new();
    restarted_service.observe(WindowEvent::Discovered(window("unknown-first")));
    restarted_service.observe(WindowEvent::Discovered(window("current")));
    restarted_service.observe(WindowEvent::Discovered(window("unknown-second")));

    restarted_service.observe(WindowEvent::Activated(window("current")));
    restarted_service.complete_initial_discovery();

    assert_eq!(
        restarted_service.diagnostics(),
        ServiceDiagnostics {
            mru_history: MruHistoryAccuracy::WarmUp,
            mru_order: vec![
                window("current"),
                window("unknown-first"),
                window("unknown-second"),
            ],
            window_scope: WindowScope::AllWorkspaces,
            workspace_eligibility: WorkspaceEligibilityState::AwaitingSnapshot,
            capture_backend: CaptureBackendSelection::default(),
        }
    );
}

#[test]
fn reused_identity_does_not_inherit_the_closed_windows_recency() {
    let mut service = SwitcherService::new();
    service.observe(WindowEvent::Discovered(window("reused")));
    service.observe(WindowEvent::Activated(window("reused")));
    service.observe(WindowEvent::Discovered(window("survivor")));
    service.observe(WindowEvent::Activated(window("survivor")));
    service.complete_initial_discovery();

    service.observe(WindowEvent::Closed(window("reused")));
    service.observe(WindowEvent::Discovered(window("reused")));

    assert_eq!(
        service.diagnostics(),
        ServiceDiagnostics {
            mru_history: MruHistoryAccuracy::Accurate,
            mru_order: vec![window("survivor"), window("reused")],
            window_scope: WindowScope::AllWorkspaces,
            workspace_eligibility: WorkspaceEligibilityState::AwaitingSnapshot,
            capture_backend: CaptureBackendSelection::default(),
        }
    );
}

#[test]
fn newly_discovered_window_gets_deterministic_placement_without_losing_accuracy() {
    let mut service = SwitcherService::new();
    service.observe(WindowEvent::Discovered(window("current")));
    service.observe(WindowEvent::Activated(window("current")));
    service.complete_initial_discovery();

    service.observe(WindowEvent::Discovered(window("new-background-window")));

    assert_eq!(
        service.diagnostics(),
        ServiceDiagnostics {
            mru_history: MruHistoryAccuracy::Accurate,
            mru_order: vec![window("current"), window("new-background-window")],
            window_scope: WindowScope::AllWorkspaces,
            workspace_eligibility: WorkspaceEligibilityState::AwaitingSnapshot,
            capture_backend: CaptureBackendSelection::default(),
        }
    );
}

#[test]
fn status_diagnostics_report_mru_warm_up_without_window_titles() {
    let diagnostics = ServiceDiagnostics {
        mru_history: MruHistoryAccuracy::WarmUp,
        mru_order: vec![window("opaque-a"), window("opaque-b")],
        window_scope: WindowScope::AllWorkspaces,
        workspace_eligibility: WorkspaceEligibilityState::MissingToplevelMembership {
            advertised_version: 3,
        },
        capture_backend: CaptureBackendSelection::default(),
    };

    assert_eq!(
        diagnostics.to_string(),
        "service: running\ncapture_backend: shared-memory\ncapture_backend_fallback: DMA-BUF renderer import failed\nmru_history: warm-up\nwindow_count: 2\nwindow_scope: all-workspaces\nworkspace_filtering: not-required\nworkspace_eligibility: unavailable\nworkspace_eligibility_failure: zcosmic_toplevel_info_v1 v3 emitted no committed ext-workspace membership snapshot\nmru_order:\n  1. opaque-a\n  2. opaque-b"
    );
}

#[test]
fn status_diagnostics_are_available_in_spanish() {
    let diagnostics = ServiceDiagnostics {
        mru_history: MruHistoryAccuracy::Accurate,
        mru_order: vec![window("opaque-a")],
        window_scope: WindowScope::AllWorkspaces,
        workspace_eligibility: WorkspaceEligibilityState::Ready,
        capture_backend: CaptureBackendSelection::default(),
    };

    let localized = diagnostics.localized(Locale::Spanish);

    assert!(localized.contains("servicio: activo"));
    assert!(localized.contains("cantidad_de_ventanas: 1"));
    assert!(localized.contains("elegibilidad_de_espacios: listo"));
    assert!(!localized.contains("service: running"));
}

#[test]
fn status_diagnostics_report_missing_or_incompatible_workspace_protocols() {
    let missing = ServiceDiagnostics {
        mru_history: MruHistoryAccuracy::WarmUp,
        mru_order: Vec::new(),
        window_scope: WindowScope::AllWorkspaces,
        workspace_eligibility: WorkspaceEligibilityState::MissingToplevelInfo {
            advertised_version: None,
            required_version: 3,
        },
        capture_backend: CaptureBackendSelection::default(),
    };
    let incompatible = ServiceDiagnostics {
        mru_history: MruHistoryAccuracy::Accurate,
        mru_order: Vec::new(),
        window_scope: WindowScope::AllWorkspaces,
        workspace_eligibility: WorkspaceEligibilityState::MissingWorkspaceProtocol {
            advertised_version: Some(0),
            required_version: 1,
        },
        capture_backend: CaptureBackendSelection::default(),
    };

    assert!(missing.to_string().contains(
        "workspace_eligibility_failure: zcosmic_toplevel_info_v1 not-advertised; v3 required"
    ));
    assert!(
        incompatible
            .to_string()
            .contains("workspace_eligibility_failure: ext_workspace_manager_v1 v0; v1 required")
    );
}
