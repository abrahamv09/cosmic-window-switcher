// SPDX-License-Identifier: GPL-3.0-only

use cosmic_window_switcher::{
    DesktopSnapshot, SessionDisplay, SurfaceRole, WindowId, WindowSnapshot, WorkspaceGroupSnapshot,
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
        role: SurfaceRole::Window,
        workspace_membership: workspaces.iter().copied().map(workspace).collect(),
        output_membership: outputs.iter().copied().map(display).collect(),
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
        .switching_context([
            window("focused"),
            window("hidden-workspace"),
            window("other-display"),
        ])
        .expect("the initially focused Window has a Session Display");

    assert_eq!(
        context.eligible_windows,
        vec![window("focused"), window("other-display")]
    );
    assert_eq!(context.session_display, display("HDMI-A-1"));
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
        .switching_context([
            window("focused"),
            window("external-hidden"),
            window("external"),
            window("laptop-hidden"),
        ])
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
    let mut dialog = task_window("dialog", &["visible"], &["eDP-1"]);
    dialog.role = SurfaceRole::Dialog;
    let mut utility = task_window("utility", &["visible"], &["eDP-1"]);
    utility.role = SurfaceRole::Utility;
    let shell_surfaces = [
        ("panel", SurfaceRole::Panel),
        ("dock", SurfaceRole::Dock),
        ("menu", SurfaceRole::Menu),
        ("notification", SurfaceRole::Notification),
        ("overlay", SurfaceRole::Overlay),
    ]
    .map(|(id, role)| {
        let mut surface = task_window(id, &["visible"], &["eDP-1"]);
        surface.role = role;
        surface
    });
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
        windows: [minimized, fullscreen, dialog, utility]
            .into_iter()
            .chain(shell_surfaces)
            .collect(),
    };

    let context = snapshot
        .switching_context([
            window("minimized-native"),
            window("fullscreen-xwayland"),
            window("dialog"),
            window("utility"),
            window("panel"),
            window("dock"),
            window("menu"),
            window("notification"),
            window("overlay"),
        ])
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
fn a_live_workspace_policy_change_affects_the_next_context_only() {
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
        .switching_context(mru_order.clone())
        .expect("the focused Window identifies the Session Display");

    snapshot.workspaces[0].active = false;
    snapshot.workspaces[1].active = true;
    let second_context = snapshot
        .switching_context(mru_order)
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
