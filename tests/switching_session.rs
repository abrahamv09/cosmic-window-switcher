// SPDX-License-Identifier: GPL-3.0-only

use cosmic_window_switcher::{
    GridNavigationDirection, HoldModifiers, InvocationDirection, SessionEffect, SwitchingEvent,
    SwitchingSession, WindowId,
};

fn two_window_session() -> SwitchingSession {
    SwitchingSession::new(
        [WindowId::from("focused"), WindowId::from("previous")],
        InvocationDirection::Next,
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
fn reverse_session_initially_selects_final_mru_window() {
    let session = SwitchingSession::new(
        [
            WindowId::from("focused"),
            WindowId::from("previous"),
            WindowId::from("least-recent"),
        ],
        InvocationDirection::Previous,
        HoldModifiers::ALT,
    )
    .expect("three Windows can start a Switching Session");

    assert_eq!(session.selected(), &WindowId::from("least-recent"));
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
fn reverse_navigation_wraps_without_reordering_the_session_window_set() {
    let mut session = SwitchingSession::new(
        [
            WindowId::from("focused"),
            WindowId::from("previous"),
            WindowId::from("least-recent"),
        ],
        InvocationDirection::Next,
        HoldModifiers::ALT,
    )
    .expect("three Windows can start a Switching Session");

    assert_eq!(
        session.handle(SwitchingEvent::Navigate(InvocationDirection::Previous)),
        SessionEffect::SelectionChanged(WindowId::from("focused"))
    );
    assert_eq!(
        session.handle(SwitchingEvent::Navigate(InvocationDirection::Previous)),
        SessionEffect::SelectionChanged(WindowId::from("least-recent"))
    );
    assert_eq!(
        session.windows(),
        [
            WindowId::from("focused"),
            WindowId::from("previous"),
            WindowId::from("least-recent"),
        ]
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
fn releasing_shift_while_an_initial_hold_modifier_remains_does_not_commit() {
    let mut session = two_window_session();

    assert_eq!(
        session.handle(SwitchingEvent::HoldModifiersChanged(HoldModifiers::ALT)),
        SessionEffect::None
    );
    assert_eq!(session.selected(), &WindowId::from("previous"));
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

#[test]
fn enter_activates_the_selected_window_in_latch_mode() {
    let mut session = SwitchingSession::new(
        [WindowId::from("focused"), WindowId::from("previous")],
        InvocationDirection::Next,
        HoldModifiers::empty(),
    )
    .expect("two Windows can start a Switching Session");

    assert_eq!(
        session.handle(SwitchingEvent::Enter),
        SessionEffect::Activate(WindowId::from("previous"))
    );
}

#[test]
fn horizontal_arrows_follow_continuous_row_order_and_vertical_arrows_stay_spatial() {
    let mut session = SwitchingSession::new(
        (0..7).map(|index| WindowId::from(format!("window-{index}"))),
        InvocationDirection::Next,
        HoldModifiers::empty(),
    )
    .expect("seven Windows can start a Switching Session");

    assert_eq!(session.selected(), &WindowId::from("window-1"));
    assert_eq!(
        session.handle(SwitchingEvent::NavigateGrid {
            direction: GridNavigationDirection::Right,
            columns: 3,
        }),
        SessionEffect::SelectionChanged(WindowId::from("window-2"))
    );
    assert_eq!(
        session.handle(SwitchingEvent::NavigateGrid {
            direction: GridNavigationDirection::Right,
            columns: 3,
        }),
        SessionEffect::SelectionChanged(WindowId::from("window-3"))
    );
    assert_eq!(
        session.handle(SwitchingEvent::NavigateGrid {
            direction: GridNavigationDirection::Down,
            columns: 3,
        }),
        SessionEffect::SelectionChanged(WindowId::from("window-6"))
    );
    assert_eq!(
        session.handle(SwitchingEvent::NavigateGrid {
            direction: GridNavigationDirection::Left,
            columns: 3,
        }),
        SessionEffect::SelectionChanged(WindowId::from("window-5"))
    );
    assert_eq!(
        session.handle(SwitchingEvent::NavigateGrid {
            direction: GridNavigationDirection::Up,
            columns: 3,
        }),
        SessionEffect::SelectionChanged(WindowId::from("window-2"))
    );
    assert_eq!(
        session.handle(SwitchingEvent::NavigateGrid {
            direction: GridNavigationDirection::Right,
            columns: 3,
        }),
        SessionEffect::SelectionChanged(WindowId::from("window-3"))
    );
}

#[test]
fn arrow_navigation_normalizes_an_unavailable_column_count() {
    let mut session = two_window_session();

    assert_eq!(
        session.handle(SwitchingEvent::NavigateGrid {
            direction: GridNavigationDirection::Up,
            columns: 0,
        }),
        SessionEffect::SelectionChanged(WindowId::from("focused"))
    );
}
