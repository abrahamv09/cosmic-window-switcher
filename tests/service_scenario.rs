// SPDX-License-Identifier: GPL-3.0-only

use cosmic_window_switcher::{
    CaptureEffect, CaptureSessionModel, CardSize, DesktopSnapshot, GridNavigationDirection,
    HoldModifiers, InvocationDirection, InvocationRequest, RefreshCeiling, ServiceEffect,
    ServiceEvent, SessionDisplay, SessionInterruption, SessionLifecycleModel,
    SessionLifecycleSignal, SwitcherGrid, SwitcherItem, SwitcherService, SwitchingEvent,
    WindowEvent, WindowId, WindowScope, WindowSnapshot, WorkspaceEligibilityState,
    WorkspaceGroupSnapshot, WorkspaceId, WorkspaceSnapshot,
};

fn window(id: &str) -> WindowId {
    WindowId::from(id)
}

fn service_with_mru_order(ids: &[&str]) -> SwitcherService {
    let mut service = SwitcherService::new();
    for id in ids.iter().rev() {
        service.observe(WindowEvent::Discovered(window(id)));
        service.observe(WindowEvent::Activated(window(id)));
    }
    service
}

struct FakePresentationAdapter {
    grid: SwitcherGrid,
    captures: CaptureSessionModel,
}

impl FakePresentationAdapter {
    fn observe(&mut self, effects: Vec<ServiceEffect>) -> Vec<CaptureEffect> {
        for effect in effects {
            if let ServiceEffect::SelectionChanged(selected) = effect {
                self.grid
                    .select(&selected)
                    .expect("the service selection belongs to the Switcher Grid");
            }
        }
        let layout = self.grid.layout(760, 548, CardSize::Medium);
        self.captures
            .set_visible(self.grid.visible_windows(&layout))
    }
}

#[test]
fn quick_modifier_release_activates_without_revealing_the_overlay() {
    let mut service = service_with_mru_order(&["focused", "previous", "least-recent"]);

    assert_eq!(
        service.invoke(InvocationRequest {
            direction: InvocationDirection::Next,
            initial_hold_modifiers: HoldModifiers::ALT,
        }),
        vec![ServiceEffect::PrepareInvisibleOverlay {
            selected: window("previous"),
        }]
    );
    assert_eq!(
        service.handle(ServiceEvent::HoldModifiersChanged(HoldModifiers::empty())),
        Vec::<ServiceEffect>::new()
    );
    assert_eq!(
        service.handle(ServiceEvent::SessionReady),
        vec![ServiceEffect::Activate(window("previous"))]
    );
}

#[test]
fn reverse_invocation_reveals_with_the_final_mru_window_selected() {
    let mut service = service_with_mru_order(&["focused", "previous", "least-recent"]);

    assert_eq!(
        service.invoke(InvocationRequest {
            direction: InvocationDirection::Previous,
            initial_hold_modifiers: HoldModifiers::ALT,
        }),
        vec![ServiceEffect::PrepareInvisibleOverlay {
            selected: window("least-recent"),
        }]
    );
    assert_eq!(
        service.handle(ServiceEvent::SessionReady),
        Vec::<ServiceEffect>::new()
    );
    assert_eq!(
        service.handle(ServiceEvent::RevealDelayElapsed),
        vec![ServiceEffect::RevealOverlay {
            selected: window("least-recent"),
        }]
    );
}

#[test]
fn one_eligible_window_is_a_no_op_that_preserves_focus() {
    let mut service = service_with_mru_order(&["focused"]);

    assert_eq!(
        service.invoke(InvocationRequest {
            direction: InvocationDirection::Next,
            initial_hold_modifiers: HoldModifiers::ALT,
        }),
        Vec::<ServiceEffect>::new()
    );
}

#[test]
fn losing_every_window_cancels_the_preparing_session() {
    let mut service = service_with_mru_order(&["focused", "previous"]);
    service.invoke(InvocationRequest {
        direction: InvocationDirection::Next,
        initial_hold_modifiers: HoldModifiers::ALT,
    });

    assert_eq!(
        service.observe(WindowEvent::Closed(window("focused"))),
        Vec::<ServiceEffect>::new()
    );
    assert_eq!(
        service.observe(WindowEvent::Closed(window("previous"))),
        vec![ServiceEffect::Cancel]
    );
}

#[test]
fn losing_the_selected_window_advances_to_the_next_surviving_window() {
    let mut service = service_with_mru_order(&["focused", "selected", "next-survivor"]);
    service.invoke(InvocationRequest {
        direction: InvocationDirection::Next,
        initial_hold_modifiers: HoldModifiers::ALT,
    });

    assert_eq!(
        service.observe(WindowEvent::Closed(window("selected"))),
        vec![ServiceEffect::SelectionChanged(window("next-survivor"))]
    );
    service.handle(ServiceEvent::SessionReady);
    assert_eq!(
        service.handle(ServiceEvent::HoldModifiersChanged(HoldModifiers::empty())),
        vec![ServiceEffect::Activate(window("next-survivor"))]
    );
}

#[test]
fn every_desktop_interruption_cancels_without_activating_and_discards_session_state() {
    for interruption in [
        SessionInterruption::ScreenLock,
        SessionInterruption::Suspend,
        SessionInterruption::UserSwitch,
        SessionInterruption::OutputLoss,
        SessionInterruption::CompositorLoss,
        SessionInterruption::SessionShutdown,
    ] {
        let mut service = service_with_mru_order(&["focused", "selected", "survivor"]);
        service.invoke(InvocationRequest {
            direction: InvocationDirection::Next,
            initial_hold_modifiers: HoldModifiers::ALT,
        });
        service.handle(ServiceEvent::SessionReady);
        service.handle(ServiceEvent::RevealDelayElapsed);

        assert_eq!(
            service.handle(ServiceEvent::SessionInterrupted(interruption)),
            vec![ServiceEffect::Cancel],
            "{interruption:?}"
        );
        assert_eq!(
            service.handle(ServiceEvent::HoldModifiersChanged(HoldModifiers::empty())),
            Vec::<ServiceEffect>::new(),
            "{interruption:?} must not resurrect or activate the old selection"
        );
    }
}

#[test]
fn mru_observation_stops_during_session_deactivation_and_rebuilds_after_resume() {
    let mut service = service_with_mru_order(&["focused", "previous"]);

    service.handle(ServiceEvent::SessionInterrupted(
        SessionInterruption::ScreenLock,
    ));
    service.observe(WindowEvent::Activated(window("previous")));
    assert!(service.diagnostics().mru_order.is_empty());
    assert!(
        service
            .invoke(InvocationRequest {
                direction: InvocationDirection::Next,
                initial_hold_modifiers: HoldModifiers::ALT,
            })
            .is_empty()
    );

    service.handle(ServiceEvent::SessionReactivated);
    service.observe(WindowEvent::Discovered(window("focused-after-resume")));
    service.observe(WindowEvent::Discovered(window("previous-after-resume")));
    service.observe(WindowEvent::Activated(window("focused-after-resume")));
    service.complete_initial_discovery();

    assert_eq!(
        service.diagnostics().mru_order,
        vec![
            window("focused-after-resume"),
            window("previous-after-resume")
        ]
    );
}

#[test]
fn output_loss_preserves_mru_observation_for_the_still_active_cosmic_session() {
    let mut service = service_with_mru_order(&["focused", "previous"]);

    service.handle(ServiceEvent::SessionInterrupted(
        SessionInterruption::OutputLoss,
    ));
    service.observe(WindowEvent::Activated(window("previous")));

    assert_eq!(
        service.diagnostics().mru_order,
        vec![window("previous"), window("focused")]
    );
}

#[test]
fn output_loss_before_visible_reveal_delegates_to_the_stock_switcher() {
    let mut service = service_with_mru_order(&["focused", "previous"]);
    service.invoke(InvocationRequest {
        direction: InvocationDirection::Previous,
        initial_hold_modifiers: HoldModifiers::ALT,
    });

    assert_eq!(
        service.handle(ServiceEvent::SessionInterrupted(
            SessionInterruption::OutputLoss,
        )),
        vec![ServiceEffect::FallbackToStockSwitcher(
            InvocationDirection::Previous
        )]
    );
}

#[test]
fn lifecycle_sources_cannot_reactivate_until_every_privacy_boundary_clears() {
    let mut lifecycle = SessionLifecycleModel::new();

    assert_eq!(
        lifecycle.handle(SessionLifecycleSignal::Locked(true)),
        Some(ServiceEvent::SessionInterrupted(
            SessionInterruption::ScreenLock
        ))
    );
    assert_eq!(
        lifecycle.handle(SessionLifecycleSignal::PreparingForSleep(true)),
        None
    );
    assert_eq!(
        lifecycle.handle(SessionLifecycleSignal::PreparingForSleep(false)),
        None,
        "unlock is still required after resume"
    );
    assert_eq!(
        lifecycle.handle(SessionLifecycleSignal::Locked(false)),
        Some(ServiceEvent::SessionReactivated)
    );
    assert_eq!(
        lifecycle.handle(SessionLifecycleSignal::Active(false)),
        Some(ServiceEvent::SessionInterrupted(
            SessionInterruption::UserSwitch
        ))
    );
    assert_eq!(
        lifecycle.handle(SessionLifecycleSignal::Active(true)),
        Some(ServiceEvent::SessionReactivated)
    );
    assert_eq!(
        lifecycle.handle(SessionLifecycleSignal::PreparingForShutdown(true)),
        Some(ServiceEvent::SessionInterrupted(
            SessionInterruption::SessionShutdown
        ))
    );
}

#[test]
fn failed_session_readiness_falls_back_in_the_requested_direction() {
    let mut service = service_with_mru_order(&["focused", "previous"]);
    service.invoke(InvocationRequest {
        direction: InvocationDirection::Previous,
        initial_hold_modifiers: HoldModifiers::ALT,
    });

    assert_eq!(
        service.handle(ServiceEvent::SessionReadinessFailed),
        vec![ServiceEffect::FallbackToStockSwitcher(
            InvocationDirection::Previous
        )]
    );
}

#[test]
fn all_workspaces_does_not_fall_back_when_workspace_membership_is_missing() {
    let mut service = service_with_mru_order(&["focused", "previous"]);
    service.set_workspace_eligibility_state(WorkspaceEligibilityState::MissingToplevelMembership {
        advertised_version: 3,
    });

    assert_eq!(
        service.workspace_invocation_fallback(InvocationDirection::Next),
        None
    );
}

#[test]
fn visible_workspaces_falls_back_in_each_direction_when_membership_is_missing() {
    let mut service = service_with_mru_order(&["focused", "previous"]);
    service.set_window_scope(WindowScope::VisibleWorkspaces);
    service.set_workspace_eligibility_state(WorkspaceEligibilityState::MissingToplevelMembership {
        advertised_version: 3,
    });

    assert_eq!(
        service.workspace_invocation_fallback(InvocationDirection::Next),
        Some(ServiceEffect::FallbackToStockSwitcher(
            InvocationDirection::Next
        ))
    );
    assert_eq!(
        service.workspace_invocation_fallback(InvocationDirection::Previous),
        Some(ServiceEffect::FallbackToStockSwitcher(
            InvocationDirection::Previous
        ))
    );
}

#[test]
fn repeated_invocations_move_in_their_requested_direction() {
    let mut service = service_with_mru_order(&["focused", "previous", "least-recent"]);
    service.invoke(InvocationRequest {
        direction: InvocationDirection::Next,
        initial_hold_modifiers: HoldModifiers::ALT,
    });

    assert_eq!(
        service.handle(ServiceEvent::Invocation(InvocationDirection::Next)),
        vec![ServiceEffect::SelectionChanged(window("least-recent"))]
    );
    assert_eq!(
        service.handle(ServiceEvent::Invocation(InvocationDirection::Previous)),
        vec![ServiceEffect::SelectionChanged(window("previous"))]
    );
}

#[test]
fn spatial_keyboard_navigation_updates_the_active_service_selection() {
    let mut service = service_with_mru_order(&[
        "window-0", "window-1", "window-2", "window-3", "window-4", "window-5",
    ]);
    service.invoke(InvocationRequest {
        direction: InvocationDirection::Next,
        initial_hold_modifiers: HoldModifiers::ALT,
    });

    assert_eq!(
        service.handle(ServiceEvent::Switching(SwitchingEvent::NavigateGrid {
            direction: GridNavigationDirection::Right,
            columns: 3,
        })),
        vec![ServiceEffect::SelectionChanged(window("window-2"))]
    );
    assert_eq!(
        service.handle(ServiceEvent::Switching(SwitchingEvent::NavigateGrid {
            direction: GridNavigationDirection::Down,
            columns: 3,
        })),
        vec![ServiceEffect::SelectionChanged(window("window-5"))]
    );
}

#[test]
fn latch_mode_stays_open_until_enter_activates() {
    let mut service = service_with_mru_order(&["focused", "previous"]);
    service.invoke(InvocationRequest {
        direction: InvocationDirection::Next,
        initial_hold_modifiers: HoldModifiers::empty(),
    });
    service.handle(ServiceEvent::SessionReady);
    service.handle(ServiceEvent::RevealDelayElapsed);

    assert_eq!(
        service.handle(ServiceEvent::HoldModifiersChanged(HoldModifiers::empty())),
        Vec::<ServiceEffect>::new()
    );
    assert_eq!(
        service.handle(ServiceEvent::Switching(
            cosmic_window_switcher::SwitchingEvent::Enter
        )),
        vec![ServiceEffect::Activate(window("previous"))]
    );
}

#[test]
fn escape_cancels_without_requesting_a_focus_change() {
    let mut service = service_with_mru_order(&["focused", "previous"]);
    service.invoke(InvocationRequest {
        direction: InvocationDirection::Next,
        initial_hold_modifiers: HoldModifiers::ALT,
    });
    service.handle(ServiceEvent::SessionReady);
    service.handle(ServiceEvent::RevealDelayElapsed);

    assert_eq!(
        service.handle(ServiceEvent::Switching(
            cosmic_window_switcher::SwitchingEvent::Escape
        )),
        vec![ServiceEffect::Cancel]
    );
}

#[test]
fn windows_discovered_during_switching_wait_for_the_next_session() {
    let mut service = service_with_mru_order(&["focused", "previous", "least-recent"]);
    service.invoke(InvocationRequest {
        direction: InvocationDirection::Next,
        initial_hold_modifiers: HoldModifiers::ALT,
    });

    service.observe(WindowEvent::Discovered(window("new")));
    service.observe(WindowEvent::Activated(window("new")));

    assert_eq!(
        service.handle(ServiceEvent::Invocation(InvocationDirection::Next)),
        vec![ServiceEffect::SelectionChanged(window("least-recent"))]
    );
}

#[test]
fn prepared_session_uses_the_window_set_captured_with_the_invocation_request() {
    let mut service = service_with_mru_order(&["focused", "previous", "least-recent"]);
    let session_window_set = service.diagnostics().mru_order;
    service.observe(WindowEvent::Discovered(window("new")));
    service.observe(WindowEvent::Activated(window("new")));

    assert_eq!(
        service.invoke_for_window_set(
            InvocationRequest {
                direction: InvocationDirection::Next,
                initial_hold_modifiers: HoldModifiers::ALT,
            },
            session_window_set,
        ),
        vec![ServiceEffect::PrepareInvisibleOverlay {
            selected: window("previous"),
        }]
    );
    assert_eq!(
        service.handle(ServiceEvent::Invocation(InvocationDirection::Previous)),
        vec![ServiceEffect::SelectionChanged(window("focused"))]
    );
}

#[test]
fn pointer_entry_is_inert_until_motion_then_hover_selects_and_press_activates() {
    let mut service = service_with_mru_order(&["focused", "previous", "least-recent"]);
    service.invoke(InvocationRequest {
        direction: InvocationDirection::Next,
        initial_hold_modifiers: HoldModifiers::empty(),
    });
    service.handle(ServiceEvent::SessionReady);
    service.handle(ServiceEvent::RevealDelayElapsed);

    assert_eq!(
        service.handle(ServiceEvent::PointerEntered(Some(window("least-recent")))),
        Vec::<ServiceEffect>::new()
    );
    assert_eq!(
        service.handle(ServiceEvent::PointerMoved(Some(window("least-recent")))),
        vec![ServiceEffect::SelectionChanged(window("least-recent"))]
    );
    assert_eq!(
        service.handle(ServiceEvent::PointerPressed(Some(window("least-recent")))),
        Vec::<ServiceEffect>::new()
    );
    assert_eq!(
        service.handle(ServiceEvent::PointerReleased(Some(window("least-recent")))),
        vec![ServiceEffect::Activate(window("least-recent"))]
    );
}

#[test]
fn pointer_press_outside_the_revealed_grid_cancels_without_activation() {
    let mut service = service_with_mru_order(&["focused", "previous"]);
    service.invoke(InvocationRequest {
        direction: InvocationDirection::Next,
        initial_hold_modifiers: HoldModifiers::empty(),
    });

    assert_eq!(
        service.handle(ServiceEvent::PointerPressed(None)),
        Vec::<ServiceEffect>::new()
    );
    service.handle(ServiceEvent::SessionReady);
    service.handle(ServiceEvent::RevealDelayElapsed);

    assert_eq!(
        service.handle(ServiceEvent::PointerPressed(None)),
        Vec::<ServiceEffect>::new()
    );
    assert_eq!(
        service.handle(ServiceEvent::PointerReleased(None)),
        vec![ServiceEffect::Cancel]
    );
}

#[test]
fn moving_away_before_pointer_release_withdraws_the_click() {
    let mut service = service_with_mru_order(&["focused", "previous", "least-recent"]);
    service.invoke(InvocationRequest {
        direction: InvocationDirection::Next,
        initial_hold_modifiers: HoldModifiers::empty(),
    });
    service.handle(ServiceEvent::SessionReady);
    service.handle(ServiceEvent::RevealDelayElapsed);

    assert_eq!(
        service.handle(ServiceEvent::PointerPressed(Some(window("least-recent")))),
        Vec::<ServiceEffect>::new()
    );
    assert_eq!(
        service.handle(ServiceEvent::PointerReleased(None)),
        Vec::<ServiceEffect>::new()
    );
    assert_eq!(
        service.handle(ServiceEvent::Switching(
            cosmic_window_switcher::SwitchingEvent::Enter
        )),
        vec![ServiceEffect::Activate(window("previous"))]
    );
}

#[test]
fn keyboard_navigation_suspends_offscreen_capture_and_resumes_rows_when_revealed() {
    let ids = (0..8)
        .map(|index| format!("window-{index}"))
        .collect::<Vec<_>>();
    let id_refs = ids.iter().map(String::as_str).collect::<Vec<_>>();
    let mut service = service_with_mru_order(&id_refs);
    let mut presentation = FakePresentationAdapter {
        grid: SwitcherGrid::new(
            SessionDisplay::from("eDP-1"),
            ids.iter().map(|id| {
                SwitcherItem::new(window(id), "com.example.Application".to_owned(), id.clone())
            }),
            &window("window-1"),
        )
        .expect("the Initial Selection belongs to the Switcher Grid"),
        captures: CaptureSessionModel::new(RefreshCeiling::Fps30),
    };
    service.invoke(InvocationRequest {
        direction: InvocationDirection::Next,
        initial_hold_modifiers: HoldModifiers::ALT,
    });
    assert_eq!(
        presentation.observe(Vec::new()),
        (0..4)
            .map(|index| CaptureEffect::CreateStream(window(&format!("window-{index}"))))
            .collect::<Vec<_>>()
    );

    let mut forward_capture_effects = Vec::new();
    for _ in 0..5 {
        let effects = service.handle(ServiceEvent::Invocation(InvocationDirection::Next));
        forward_capture_effects.extend(presentation.observe(effects));
    }
    assert_eq!(
        forward_capture_effects,
        [
            CaptureEffect::ReleaseStream(window("window-0")),
            CaptureEffect::ReleaseStream(window("window-1")),
            CaptureEffect::CreateStream(window("window-4")),
            CaptureEffect::CreateStream(window("window-5")),
            CaptureEffect::ReleaseStream(window("window-2")),
            CaptureEffect::ReleaseStream(window("window-3")),
            CaptureEffect::CreateStream(window("window-6")),
            CaptureEffect::CreateStream(window("window-7")),
        ]
    );

    let mut reverse_capture_effects = Vec::new();
    for _ in 0..5 {
        let effects = service.handle(ServiceEvent::Invocation(InvocationDirection::Previous));
        reverse_capture_effects.extend(presentation.observe(effects));
    }
    assert_eq!(
        reverse_capture_effects,
        [
            CaptureEffect::ReleaseStream(window("window-6")),
            CaptureEffect::ReleaseStream(window("window-7")),
            CaptureEffect::CreateStream(window("window-2")),
            CaptureEffect::CreateStream(window("window-3")),
            CaptureEffect::ReleaseStream(window("window-4")),
            CaptureEffect::ReleaseStream(window("window-5")),
            CaptureEffect::CreateStream(window("window-0")),
            CaptureEffect::CreateStream(window("window-1")),
        ]
    );
}

#[test]
fn minimized_and_fullscreen_mixed_windows_use_one_activation_behavior() {
    let visible_workspace = WorkspaceId::from("visible");
    let display = SessionDisplay::from("eDP-1");
    let desktop = DesktopSnapshot {
        workspace_groups: vec![WorkspaceGroupSnapshot {
            outputs: vec![display.clone()],
            workspaces: vec![visible_workspace.clone()],
        }],
        workspaces: vec![WorkspaceSnapshot {
            id: visible_workspace.clone(),
            active: true,
            hidden: false,
        }],
        windows: vec![
            WindowSnapshot {
                id: window("fullscreen-native"),
                workspace_membership: vec![visible_workspace.clone()],
                output_membership: vec![display.clone()],
                session_display: Some(display.clone()),
                minimized: false,
                fullscreen: true,
                sticky: false,
            },
            WindowSnapshot {
                id: window("minimized-xwayland"),
                workspace_membership: vec![visible_workspace],
                output_membership: vec![display],
                session_display: Some(SessionDisplay::from("eDP-1")),
                minimized: true,
                fullscreen: false,
                sticky: false,
            },
        ],
    };
    let context = desktop
        .switching_context(
            WindowScope::VisibleWorkspaces,
            [window("fullscreen-native"), window("minimized-xwayland")],
        )
        .expect("the focused Window identifies the Session Display");
    let mut service = service_with_mru_order(&["fullscreen-native", "minimized-xwayland"]);

    assert_eq!(
        service.invoke_for_window_set(
            InvocationRequest {
                direction: InvocationDirection::Next,
                initial_hold_modifiers: HoldModifiers::ALT,
            },
            context.eligible_windows,
        ),
        vec![ServiceEffect::PrepareInvisibleOverlay {
            selected: window("minimized-xwayland"),
        }]
    );
    service.handle(ServiceEvent::SessionReady);
    assert_eq!(
        service.handle(ServiceEvent::HoldModifiersChanged(HoldModifiers::empty())),
        vec![ServiceEffect::Activate(window("minimized-xwayland"))]
    );
}
