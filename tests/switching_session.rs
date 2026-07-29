// SPDX-License-Identifier: GPL-3.0-only

use cosmic_window_switcher::{
    HoldModifiers, SessionEffect, SwitchingEvent, SwitchingSession, WindowId,
};

fn two_window_session() -> SwitchingSession {
    SwitchingSession::new(
        [WindowId::from("focused"), WindowId::from("previous")],
        HoldModifiers::ALT,
    )
    .expect("two Windows can start a Switching Session")
}

#[test]
fn two_window_session_initially_selects_previous_window() {
    let session = two_window_session();

    assert_eq!(session.selected(), &WindowId::from("previous"));
}

#[test]
fn tab_cycles_through_the_session_window_set() {
    let mut session = two_window_session();

    assert_eq!(
        session.handle(SwitchingEvent::Tab),
        SessionEffect::SelectionChanged(WindowId::from("focused"))
    );
    assert_eq!(
        session.handle(SwitchingEvent::Tab),
        SessionEffect::SelectionChanged(WindowId::from("previous"))
    );
}

#[test]
fn escape_cancels_without_activating_a_window() {
    let mut session = two_window_session();

    assert_eq!(
        session.handle(SwitchingEvent::Escape),
        SessionEffect::Cancelled
    );
    assert_eq!(
        session.handle(SwitchingEvent::HoldModifiersChanged(HoldModifiers::empty())),
        SessionEffect::None
    );
}

#[test]
fn releasing_initial_hold_modifier_activates_selection_exactly_once() {
    let mut session = two_window_session();

    assert_eq!(
        session.handle(SwitchingEvent::HoldModifiersChanged(HoldModifiers::empty())),
        SessionEffect::Activate(WindowId::from("previous"))
    );
    assert_eq!(
        session.handle(SwitchingEvent::HoldModifiersChanged(HoldModifiers::empty())),
        SessionEffect::None
    );
}
