// SPDX-License-Identifier: GPL-3.0-only

use cosmic_window_switcher::{
    ApplicationIcon, SessionDisplay, SwitcherCard, SwitcherGrid, WindowId,
};

fn card(id: &str, application_id: &str, title: &str) -> SwitcherCard {
    SwitcherCard::new(
        WindowId::from(id),
        application_id.to_owned(),
        title.to_owned(),
    )
}

#[test]
fn grid_exposes_icon_title_and_accessible_selected_state_for_every_card() {
    let grid = SwitcherGrid::new(
        SessionDisplay::from("eDP-1"),
        [
            card("focused", "com.example.Editor", "Notes"),
            card("selected", "com.example.Terminal", "Build"),
        ],
        &WindowId::from("selected"),
    )
    .expect("the selected Window belongs to the grid");

    assert_eq!(grid.session_display(), &SessionDisplay::from("eDP-1"));
    assert_eq!(grid.cards()[0].title(), "Notes");
    assert_eq!(
        grid.cards()[0].application_icon(),
        &ApplicationIcon::Monogram('E')
    );
    assert_eq!(grid.cards()[0].accessible_name(), "Notes");
    assert_eq!(grid.cards()[0].accessible_position(), (1, 2));
    assert!(!grid.cards()[0].is_selected());
    assert_eq!(grid.cards()[1].accessible_name(), "Build");
    assert_eq!(grid.cards()[1].accessible_position(), (2, 2));
    assert!(grid.cards()[1].is_selected());
}

#[test]
fn changing_selection_updates_one_accessible_selected_state_without_reordering_cards() {
    let mut grid = SwitcherGrid::new(
        SessionDisplay::from("HDMI-A-1"),
        [
            card("focused", "com.example.Editor", "Notes"),
            card("previous", "com.example.Terminal", "Build"),
            card("least-recent", "com.example.Browser", "Reference"),
        ],
        &WindowId::from("previous"),
    )
    .expect("the selected Window belongs to the grid");

    grid.select(&WindowId::from("least-recent"))
        .expect("the selected Window belongs to the grid");

    assert_eq!(
        grid.cards()
            .iter()
            .map(|card| card.window().as_str())
            .collect::<Vec<_>>(),
        ["focused", "previous", "least-recent"]
    );
    assert_eq!(
        grid.cards()
            .iter()
            .map(SwitcherCard::is_selected)
            .collect::<Vec<_>>(),
        [false, false, true]
    );
}

#[test]
fn blank_titles_use_the_application_identity_as_the_accessible_fallback() {
    let grid = SwitcherGrid::new(
        SessionDisplay::from("eDP-1"),
        [card("window", "org.example.Files", "")],
        &WindowId::from("window"),
    )
    .expect("the selected Window belongs to the grid");

    assert_eq!(grid.cards()[0].title(), "org.example.Files");
    assert_eq!(grid.cards()[0].accessible_name(), "org.example.Files");
    assert_eq!(
        grid.cards()[0].application_icon(),
        &ApplicationIcon::Monogram('F')
    );
}

#[test]
fn closing_a_card_removes_it_without_reordering_survivors() {
    let mut grid = SwitcherGrid::new(
        SessionDisplay::from("eDP-1"),
        [
            card("focused", "com.example.Editor", "Notes"),
            card("selected", "com.example.Terminal", "Build"),
            card("survivor", "com.example.Browser", "Reference"),
        ],
        &WindowId::from("selected"),
    )
    .expect("the selected Window belongs to the grid");

    assert!(grid.remove(&WindowId::from("selected")));
    grid.select(&WindowId::from("survivor"))
        .expect("the selected Window belongs to the grid");

    assert_eq!(
        grid.cards()
            .iter()
            .map(|card| (card.window().as_str(), card.accessible_position()))
            .collect::<Vec<_>>(),
        [("focused", (1, 2)), ("survivor", (2, 2))]
    );
    assert!(grid.cards()[1].is_selected());
}
