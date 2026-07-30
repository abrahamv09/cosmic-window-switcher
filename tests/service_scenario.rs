// SPDX-License-Identifier: GPL-3.0-only

use cosmic_window_switcher::{
    HoldModifiers, InvocationDirection, InvocationRequest, ServiceEffect, ServiceEvent,
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
