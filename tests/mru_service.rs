// SPDX-License-Identifier: GPL-3.0-only

use cosmic_window_switcher::{
    MruHistoryAccuracy, ServiceDiagnostics, SwitcherService, WindowEvent, WindowId,
    WorkspaceEligibilityDiagnostics,
};

fn window(id: &str) -> WindowId {
    WindowId::from(id)
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
            workspace_eligibility: WorkspaceEligibilityDiagnostics::AwaitingSnapshot,
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
            workspace_eligibility: WorkspaceEligibilityDiagnostics::AwaitingSnapshot,
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
            workspace_eligibility: WorkspaceEligibilityDiagnostics::AwaitingSnapshot,
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
            workspace_eligibility: WorkspaceEligibilityDiagnostics::AwaitingSnapshot,
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
            workspace_eligibility: WorkspaceEligibilityDiagnostics::AwaitingSnapshot,
        }
    );
}

#[test]
fn status_diagnostics_report_mru_warm_up_without_window_titles() {
    let diagnostics = ServiceDiagnostics {
        mru_history: MruHistoryAccuracy::WarmUp,
        mru_order: vec![window("opaque-a"), window("opaque-b")],
        workspace_eligibility: WorkspaceEligibilityDiagnostics::MissingToplevelMembership {
            advertised_version: 3,
        },
    };

    assert_eq!(
        diagnostics.to_string(),
        "service: running\nmru_history: warm-up\nwindow_count: 2\nworkspace_eligibility: unavailable\nworkspace_eligibility_failure: zcosmic_toplevel_info_v1 v3 emitted no committed ext-workspace membership snapshot\nmru_order:\n  1. opaque-a\n  2. opaque-b"
    );
}
