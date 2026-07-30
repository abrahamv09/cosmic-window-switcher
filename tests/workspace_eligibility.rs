// SPDX-License-Identifier: GPL-3.0-only

use cosmic_window_switcher::{
    DesktopSnapshot, SessionDisplay, WindowId, WindowScope, WindowSnapshot, WorkspaceGroupSnapshot,
    WorkspaceId, WorkspaceSnapshot,
};

fn window(id: &str) -> WindowId {
    WindowId::from(id)
}

fn workspace(id: &str) -> WorkspaceId {
    WorkspaceId::from(id)
}

fn display(id: &str) -> SessionDisplay {
    SessionDisplay::from(id)
}

fn task_window(id: &str, workspaces: &[&str], outputs: &[&str]) -> WindowSnapshot {
    WindowSnapshot {
        id: window(id),
        workspace_membership: workspaces.iter().copied().map(workspace).collect(),
        output_membership: outputs.iter().copied().map(display).collect(),
        session_display: outputs.first().copied().map(display),
        minimized: false,
        fullscreen: false,
        sticky: false,
    }
}

#[test]
fn spanning_workspace_includes_its_windows_across_all_displays_in_mru_order() {
    let snapshot = DesktopSnapshot {
        workspace_groups: vec![WorkspaceGroupSnapshot {
            outputs: vec![display("eDP-1"), display("HDMI-A-1")],
            workspaces: vec![workspace("visible"), workspace("hidden")],
        }],
        workspaces: vec![
            WorkspaceSnapshot {
                id: workspace("visible"),
                active: true,
                hidden: false,
            },
            WorkspaceSnapshot {
                id: workspace("hidden"),
                active: false,
                hidden: false,
            },
        ],
        windows: vec![
            task_window("focused", &["visible"], &["HDMI-A-1"]),
            task_window("other-display", &["visible"], &["eDP-1"]),
            task_window("hidden-workspace", &["hidden"], &["eDP-1"]),
        ],
    };

    let context = snapshot
        .switching_context(
            WindowScope::VisibleWorkspaces,
            [
                window("focused"),
                window("hidden-workspace"),
                window("other-display"),
            ],
        )
        .expect("the initially focused Window has a Session Display");

    assert_eq!(
        context.eligible_windows,
        vec![window("focused"), window("other-display")]
    );
    assert_eq!(context.session_display, display("HDMI-A-1"));
}

#[test]
fn all_workspaces_includes_hidden_workspace_windows_and_uses_a_group_output_fallback() {
    let mut focused = task_window("focused", &[], &[]);
    focused.session_display = None;
    let snapshot = DesktopSnapshot {
        workspace_groups: vec![WorkspaceGroupSnapshot {
            outputs: vec![display("eDP-1")],
            workspaces: vec![workspace("visible"), workspace("hidden")],
        }],
        workspaces: vec![
            WorkspaceSnapshot {
                id: workspace("visible"),
                active: true,
                hidden: false,
            },
            WorkspaceSnapshot {
                id: workspace("hidden"),
                active: false,
                hidden: false,
            },
        ],
        windows: vec![
            focused,
            task_window("hidden-workspace", &[], &[]),
            task_window("visible-workspace", &[], &[]),
        ],
    };

    let context = snapshot
        .switching_context(
            WindowScope::AllWorkspaces,
            [
                window("focused"),
                window("hidden-workspace"),
                window("visible-workspace"),
            ],
        )
        .expect("an assigned workspace-group output provides the Session Display");

    assert_eq!(
        context.eligible_windows,
        vec![
            window("focused"),
            window("hidden-workspace"),
            window("visible-workspace"),
        ]
    );
    assert_eq!(context.session_display, display("eDP-1"));
}

#[test]
fn compositor_selected_session_display_wins_over_output_event_order() {
    let mut focused = task_window("focused", &["visible"], &["left", "right"]);
    focused.session_display = Some(display("right"));
    let snapshot = DesktopSnapshot {
        workspace_groups: vec![WorkspaceGroupSnapshot {
            outputs: vec![display("left"), display("right")],
            workspaces: vec![workspace("visible")],
        }],
        workspaces: vec![WorkspaceSnapshot {
            id: workspace("visible"),
            active: true,
            hidden: false,
        }],
        windows: vec![focused, task_window("previous", &["visible"], &["left"])],
    };

    let context = snapshot
        .switching_context(
            WindowScope::VisibleWorkspaces,
            [window("focused"), window("previous")],
        )
        .expect("the compositor selected a Session Display");

    assert_eq!(context.session_display, display("right"));
}

#[test]
fn separate_display_groups_include_each_active_workspace() {
    let snapshot = DesktopSnapshot {
        workspace_groups: vec![
            WorkspaceGroupSnapshot {
                outputs: vec![display("eDP-1")],
                workspaces: vec![workspace("laptop-active"), workspace("laptop-hidden")],
            },
            WorkspaceGroupSnapshot {
                outputs: vec![display("HDMI-A-1")],
                workspaces: vec![workspace("external-active"), workspace("external-hidden")],
            },
        ],
        workspaces: vec![
            WorkspaceSnapshot {
                id: workspace("laptop-active"),
                active: true,
                hidden: false,
            },
            WorkspaceSnapshot {
                id: workspace("laptop-hidden"),
                active: false,
                hidden: false,
            },
            WorkspaceSnapshot {
                id: workspace("external-active"),
                active: true,
                hidden: false,
            },
            WorkspaceSnapshot {
                id: workspace("external-hidden"),
                active: false,
                hidden: false,
            },
        ],
        windows: vec![
            task_window("focused", &["laptop-active"], &["eDP-1"]),
            task_window("external", &["external-active"], &["HDMI-A-1"]),
            task_window("laptop-hidden", &["laptop-hidden"], &["eDP-1"]),
            task_window("external-hidden", &["external-hidden"], &["HDMI-A-1"]),
        ],
    };

    let context = snapshot
        .switching_context(
            WindowScope::VisibleWorkspaces,
            [
                window("focused"),
                window("external-hidden"),
                window("external"),
                window("laptop-hidden"),
            ],
        )
        .expect("the focused Window identifies the Session Display");

    assert_eq!(
        context.eligible_windows,
        vec![window("focused"), window("external")]
    );
}

#[test]
fn minimized_dialog_utility_and_mixed_window_types_share_eligibility() {
    let mut minimized = task_window("minimized-native", &["visible"], &["eDP-1"]);
    minimized.minimized = true;
    let mut fullscreen = task_window("fullscreen-xwayland", &["visible"], &["eDP-1"]);
    fullscreen.fullscreen = true;
    let dialog = task_window("dialog", &["visible"], &["eDP-1"]);
    let utility = task_window("utility", &["visible"], &["eDP-1"]);
    let snapshot = DesktopSnapshot {
        workspace_groups: vec![WorkspaceGroupSnapshot {
            outputs: vec![display("eDP-1")],
            workspaces: vec![workspace("visible")],
        }],
        workspaces: vec![WorkspaceSnapshot {
            id: workspace("visible"),
            active: true,
            hidden: false,
        }],
        // Only independently exposed Windows enter this snapshot. COSMIC
        // layer-shell panels, docks, menus, notifications, and overlays are
        // excluded at the foreign-toplevel adapter boundary.
        windows: vec![minimized, fullscreen, dialog, utility],
    };

    let context = snapshot
        .switching_context(
            WindowScope::VisibleWorkspaces,
            [
                window("minimized-native"),
                window("fullscreen-xwayland"),
                window("dialog"),
                window("utility"),
            ],
        )
        .expect("the focused Window identifies the Session Display");

    assert_eq!(
        context.eligible_windows,
        vec![
            window("minimized-native"),
            window("fullscreen-xwayland"),
            window("dialog"),
            window("utility"),
        ]
    );
}

#[test]
fn a_cosmic_workspace_policy_change_affects_the_next_context_only() {
    let mut snapshot = DesktopSnapshot {
        workspace_groups: vec![WorkspaceGroupSnapshot {
            outputs: vec![display("eDP-1")],
            workspaces: vec![workspace("first"), workspace("second")],
        }],
        workspaces: vec![
            WorkspaceSnapshot {
                id: workspace("first"),
                active: true,
                hidden: false,
            },
            WorkspaceSnapshot {
                id: workspace("second"),
                active: false,
                hidden: false,
            },
        ],
        windows: vec![
            task_window("focused", &["first", "second"], &["eDP-1"]),
            task_window("first-only", &["first"], &["eDP-1"]),
            task_window("second-only", &["second"], &["eDP-1"]),
        ],
    };
    let mru_order = [
        window("focused"),
        window("first-only"),
        window("second-only"),
    ];
    let first_context = snapshot
        .switching_context(WindowScope::VisibleWorkspaces, mru_order.clone())
        .expect("the focused Window identifies the Session Display");

    snapshot.workspaces[0].active = false;
    snapshot.workspaces[1].active = true;
    let second_context = snapshot
        .switching_context(WindowScope::VisibleWorkspaces, mru_order)
        .expect("the focused Window identifies the Session Display");

    assert_eq!(
        first_context.eligible_windows,
        vec![window("focused"), window("first-only")]
    );
    assert_eq!(
        second_context.eligible_windows,
        vec![window("focused"), window("second-only")]
    );
}
