// SPDX-License-Identifier: GPL-3.0-only

use cosmic_window_switcher::{
    CaptureSessionModel, CardSize, HoldModifiers, InvocationDirection, InvocationRequest,
    RefreshCeiling, ServiceEffect, ServiceEvent, SessionDisplay, SwitcherGrid, SwitcherItem,
    SwitcherService, WindowEvent, WindowId,
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

fn synchronize_capture_viewport(
    grid: &mut SwitcherGrid,
    captures: &mut CaptureSessionModel,
    effects: Vec<ServiceEffect>,
) {
    for effect in effects {
        if let ServiceEffect::SelectionChanged(selected) = effect {
            grid.select(&selected)
                .expect("the service selection belongs to the Switcher Grid");
        }
    }
    let layout = grid.layout(760, 548, CardSize::Medium);
    captures.set_grid_viewport(grid, &layout);
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
    let mut grid = SwitcherGrid::new(
        SessionDisplay::from("eDP-1"),
        ids.iter().map(|id| {
            SwitcherItem::new(window(id), "com.example.Application".to_owned(), id.clone())
        }),
        &window("window-1"),
    )
    .expect("the Initial Selection belongs to the Switcher Grid");
    let mut captures = CaptureSessionModel::new(RefreshCeiling::Fps30);
    service.invoke(InvocationRequest {
        direction: InvocationDirection::Next,
        initial_hold_modifiers: HoldModifiers::ALT,
    });
    synchronize_capture_viewport(&mut grid, &mut captures, Vec::new());

    for _ in 0..5 {
        let effects = service.handle(ServiceEvent::Invocation(InvocationDirection::Next));
        synchronize_capture_viewport(&mut grid, &mut captures, effects);
    }

    assert_eq!(
        grid.items()
            .iter()
            .filter(|item| captures.is_active(item.window()))
            .map(|item| item.window().as_str())
            .collect::<Vec<_>>(),
        ["window-4", "window-5", "window-6", "window-7"]
    );

    for _ in 0..5 {
        let effects = service.handle(ServiceEvent::Invocation(InvocationDirection::Previous));
        synchronize_capture_viewport(&mut grid, &mut captures, effects);
    }

    assert_eq!(
        grid.items()
            .iter()
            .filter(|item| captures.is_active(item.window()))
            .map(|item| item.window().as_str())
            .collect::<Vec<_>>(),
        ["window-0", "window-1", "window-2", "window-3"]
    );
}
