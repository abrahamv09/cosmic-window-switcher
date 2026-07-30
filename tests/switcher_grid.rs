// SPDX-License-Identifier: GPL-3.0-only

use cosmic_window_switcher::{SessionDisplay, SwitcherGrid, SwitcherItem, WindowId};

fn item(id: &str, application_id: &str, title: &str) -> SwitcherItem {
    SwitcherItem::new(
        WindowId::from(id),
        application_id.to_owned(),
        title.to_owned(),
    )
}

#[test]
fn grid_exposes_icon_title_and_accessible_selected_state_for_every_item() {
    let grid = SwitcherGrid::new(
        SessionDisplay::from("eDP-1"),
        [
            item("focused", "com.example.Editor", "Notes"),
            item("selected", "com.example.Terminal", "Build"),
        ],
        &WindowId::from("selected"),
    )
    .expect("the selected Window belongs to the grid");

    assert_eq!(grid.session_display(), &SessionDisplay::from("eDP-1"));
    assert_eq!(grid.items()[0].title(), "Notes");
    assert_eq!(
        grid.items()[0].application_icon().name(),
        "com.example.Editor"
    );
    assert_eq!(grid.items()[0].application_icon().fallback_monogram(), 'E');
    assert_eq!(grid.items()[0].accessible_name(), "Notes");
    assert_eq!(grid.items()[0].accessible_position(), (1, 2));
    assert!(!grid.items()[0].is_selected());
    assert_eq!(grid.items()[1].accessible_name(), "Build");
    assert_eq!(grid.items()[1].accessible_position(), (2, 2));
    assert!(grid.items()[1].is_selected());
}

#[test]
fn changing_selection_updates_one_accessible_selected_state_without_reordering_items() {
    let mut grid = SwitcherGrid::new(
        SessionDisplay::from("HDMI-A-1"),
        [
            item("focused", "com.example.Editor", "Notes"),
            item("previous", "com.example.Terminal", "Build"),
            item("least-recent", "com.example.Browser", "Reference"),
        ],
        &WindowId::from("previous"),
    )
    .expect("the selected Window belongs to the grid");

    grid.select(&WindowId::from("least-recent"))
        .expect("the selected Window belongs to the grid");

    assert_eq!(
        grid.items()
            .iter()
            .map(|item| item.window().as_str())
            .collect::<Vec<_>>(),
        ["focused", "previous", "least-recent"]
    );
    assert_eq!(
        grid.items()
            .iter()
            .map(SwitcherItem::is_selected)
            .collect::<Vec<_>>(),
        [false, false, true]
    );
}

#[test]
fn blank_titles_use_the_application_identity_as_the_accessible_fallback() {
    let grid = SwitcherGrid::new(
        SessionDisplay::from("eDP-1"),
        [item("window", "org.example.Files", "")],
        &WindowId::from("window"),
    )
    .expect("the selected Window belongs to the grid");

    assert_eq!(grid.items()[0].title(), "org.example.Files");
    assert_eq!(grid.items()[0].accessible_name(), "org.example.Files");
    assert_eq!(
        grid.items()[0].application_icon().name(),
        "org.example.Files"
    );
    assert_eq!(grid.items()[0].application_icon().fallback_monogram(), 'F');
}

#[test]
fn closing_an_item_removes_it_without_reordering_survivors() {
    let mut grid = SwitcherGrid::new(
        SessionDisplay::from("eDP-1"),
        [
            item("focused", "com.example.Editor", "Notes"),
            item("selected", "com.example.Terminal", "Build"),
            item("survivor", "com.example.Browser", "Reference"),
        ],
        &WindowId::from("selected"),
    )
    .expect("the selected Window belongs to the grid");

    assert!(grid.remove(&WindowId::from("selected")));
    grid.select(&WindowId::from("survivor"))
        .expect("the selected Window belongs to the grid");

    assert_eq!(
        grid.items()
            .iter()
            .map(|item| (item.window().as_str(), item.accessible_position()))
            .collect::<Vec<_>>(),
        [("focused", (1, 2)), ("survivor", (2, 2))]
    );
    assert!(grid.items()[1].is_selected());
}
