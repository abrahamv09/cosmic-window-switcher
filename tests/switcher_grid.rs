// SPDX-License-Identifier: GPL-3.0-only

use cosmic_window_switcher::{
    CardSize, FractionalScale, GridRect, SessionDisplay, SwitcherGrid, SwitcherItem, WindowId,
};

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

#[test]
fn viewport_scrolls_only_when_the_selected_row_leaves_the_visible_rows() {
    let mut grid = SwitcherGrid::new(
        SessionDisplay::from("eDP-1"),
        (0..8).map(|index| {
            item(
                &format!("window-{index}"),
                "com.example.Application",
                &format!("Window {index}"),
            )
        }),
        &WindowId::from("window-1"),
    )
    .expect("the selected Window belongs to the grid");

    assert_eq!(grid.visible_item_range(2, 2), 0..4);

    grid.select(&WindowId::from("window-6"))
        .expect("the selected Window belongs to the grid");
    assert_eq!(grid.visible_item_range(2, 2), 4..8);

    grid.select(&WindowId::from("window-4"))
        .expect("the selected Window belongs to the grid");
    assert_eq!(grid.visible_item_range(2, 2), 4..8);

    grid.select(&WindowId::from("window-2"))
        .expect("the selected Window belongs to the grid");
    assert_eq!(grid.visible_item_range(2, 2), 2..6);
}

#[test]
fn continuous_layout_wraps_fixed_size_cards_without_pages() {
    let mut grid = SwitcherGrid::new(
        SessionDisplay::from("eDP-1"),
        (0..9).map(|index| {
            item(
                &format!("window-{index}"),
                "com.example.Application",
                &format!("Window {index}"),
            )
        }),
        &WindowId::from("window-1"),
    )
    .expect("the selected Window belongs to the grid");

    let layout = grid.layout(760, 548, CardSize::Medium);

    assert_eq!(layout.columns(), 2);
    assert_eq!(layout.total_rows(), 5);
    assert_eq!(layout.visible_rows(), 2);
    assert_eq!(layout.visible_item_range(), 0..4);
    assert_eq!(layout.logical_size(), (684, 524));
    assert_eq!(layout.item_bounds(0).map(GridRect::size), Some((320, 240)));
    assert_eq!(layout.item_bounds(3).map(GridRect::size), Some((320, 240)));
    assert_eq!(layout.item_bounds(4), None);
}

#[test]
fn overflow_layout_peeks_at_half_of_the_next_row_when_space_remains() {
    let mut grid = SwitcherGrid::new(
        SessionDisplay::from("eDP-1"),
        (0..9).map(|index| {
            item(
                &format!("window-{index}"),
                "com.example.Application",
                &format!("Window {index}"),
            )
        }),
        &WindowId::from("window-1"),
    )
    .expect("the selected Window belongs to the grid");

    let layout = grid.layout(1_316, 822, CardSize::Large);
    let peeked = layout
        .item_bounds(6)
        .expect("the first item in the next row peeks into the viewport");

    assert_eq!(layout.columns(), 3);
    assert_eq!(layout.total_rows(), 3);
    assert_eq!(layout.visible_rows(), 2);
    assert_eq!(layout.visible_item_range(), 0..9);
    assert!(layout.logical_size().1.saturating_sub(peeked.y()) >= 150);
    assert!(layout.logical_size().1.saturating_sub(peeked.y()) < 300);
}

#[test]
fn layout_reveals_the_selected_row_and_hit_tests_fractional_pointer_positions() {
    let mut grid = SwitcherGrid::new(
        SessionDisplay::from("eDP-1"),
        (0..9).map(|index| {
            item(
                &format!("window-{index}"),
                "com.example.Application",
                &format!("A very long Window title that remains item {index}"),
            )
        }),
        &WindowId::from("window-6"),
    )
    .expect("the selected Window belongs to the grid");

    let layout = grid.layout(760, 548, CardSize::Medium);

    assert_eq!(layout.visible_item_range(), 4..8);
    assert_eq!(layout.item_at(16.25, 16.75), Some(4));
    assert_eq!(layout.item_at(335.5, 20.0), Some(4));
    assert_eq!(layout.item_at(350.0, 20.0), Some(5));
    assert_eq!(layout.item_at(20.0, 270.0), Some(6));
    assert_eq!(layout.item_at(10.0, 10.0), None);
    assert_eq!(
        grid.window_at(&layout, 20.0, 270.0).map(WindowId::as_str),
        Some("window-6")
    );
}

#[test]
fn layout_can_be_centered_in_the_session_display_without_resizing_cards() {
    let mut grid = SwitcherGrid::new(
        SessionDisplay::from("eDP-1"),
        (0..4).map(|index| {
            item(
                &format!("window-{index}"),
                "com.example.Application",
                &format!("Window {index}"),
            )
        }),
        &WindowId::from("window-1"),
    )
    .expect("the selected Window belongs to the grid");

    let layout = grid
        .layout(760, 548, CardSize::Medium)
        .centered_in(1_000, 700);

    assert_eq!(
        layout.item_bounds(0).map(|bounds| (bounds.x(), bounds.y())),
        Some((174, 104))
    );
    assert_eq!(layout.item_bounds(0).map(GridRect::size), Some((320, 240)));
    assert_eq!(layout.item_at(174.25, 104.75), Some(0));
    assert_eq!(layout.item_at(20.0, 20.0), None);
}

#[test]
fn every_card_size_preset_remains_fixed_as_window_count_grows() {
    let mut grid = SwitcherGrid::new(
        SessionDisplay::from("eDP-1"),
        (0..40).map(|index| {
            item(
                &format!("window-{index}"),
                "com.example.Application",
                &format!("Window {index}"),
            )
        }),
        &WindowId::from("window-1"),
    )
    .expect("the selected Window belongs to the grid");

    for (preset, expected_size) in [
        (CardSize::Small, (240, 180)),
        (CardSize::Medium, (320, 240)),
        (CardSize::Large, (400, 300)),
    ] {
        let layout = grid.layout(1_600, 900, preset);
        assert_eq!(
            layout.item_bounds(0).map(GridRect::size),
            Some(expected_size)
        );
        assert!(layout.total_rows() > layout.visible_rows());
    }
}

#[test]
fn responsive_large_cards_follow_window_count_width_targets() {
    for (window_count, expected_size) in [
        (1, (640, 480)),
        (2, (640, 480)),
        (3, (480, 360)),
        (4, (352, 264)),
        (5, (288, 216)),
        (6, (480, 360)),
        (7, (288, 216)),
    ] {
        let mut grid = SwitcherGrid::new(
            SessionDisplay::from("eDP-1"),
            (0..window_count).map(|index| {
                item(
                    &format!("window-{index}"),
                    "com.example.Application",
                    &format!("Window {index}"),
                )
            }),
            &WindowId::from("window-0"),
        )
        .expect("the selected Window belongs to the grid");

        let layout = grid.responsive_layout(1_600, 900, CardSize::Large);

        assert_eq!(
            layout.item_bounds(0).map(GridRect::size),
            Some(expected_size)
        );
        assert_eq!(
            layout.columns(),
            match window_count {
                1..=5 => window_count,
                6 => 3,
                _ => 5,
            }
        );
    }
}

#[test]
fn overflow_layout_peeks_backward_when_selection_reaches_the_final_row() {
    let mut grid = SwitcherGrid::new(
        SessionDisplay::from("eDP-1"),
        (0..9).map(|index| {
            item(
                &format!("window-{index}"),
                "com.example.Application",
                &format!("Window {index}"),
            )
        }),
        &WindowId::from("window-7"),
    )
    .expect("the selected Window belongs to the grid");

    let layout = grid
        .layout(1_316, 822, CardSize::Large)
        .centered_in(1_316, 822);
    let viewport = layout.viewport_bounds();
    let peeked = layout
        .item_bounds(0)
        .expect("the first row peeks backward into the viewport");
    let selected = layout
        .item_bounds(7)
        .expect("the selected final row remains fully visible");

    assert_eq!(layout.visible_item_range(), 0..9);
    assert!(peeked.y() < viewport.y());
    assert!(peeked.y() + peeked.size().1 > viewport.y());
    assert!(selected.y() >= viewport.y());
    assert!(selected.y() + selected.size().1 <= viewport.y() + viewport.size().1);
}

#[test]
fn responsive_layout_fits_two_rows_and_a_half_row_on_wide_displays() {
    let mut six_windows = SwitcherGrid::new(
        SessionDisplay::from("eDP-1"),
        (0..6).map(|index| {
            item(
                &format!("six-{index}"),
                "com.example.Application",
                &format!("Window {index}"),
            )
        }),
        &WindowId::from("six-1"),
    )
    .expect("the selected Window belongs to the six-Window Grid");
    let six_layout = six_windows.responsive_layout(2_560, 1_080, CardSize::Large);

    assert_eq!(six_layout.columns(), 3);
    assert_eq!(six_layout.visible_rows(), 2);
    assert_eq!(six_layout.visible_item_range(), 0..6);
    assert!(six_layout.logical_size().1 <= 1_080 * 19 / 20);

    let mut eleven_windows = SwitcherGrid::new(
        SessionDisplay::from("eDP-1"),
        (0..11).map(|index| {
            item(
                &format!("eleven-{index}"),
                "com.example.Application",
                &format!("Window {index}"),
            )
        }),
        &WindowId::from("eleven-1"),
    )
    .expect("the selected Window belongs to the eleven-Window Grid");
    let eleven_layout = eleven_windows.responsive_layout(3_000, 900, CardSize::Large);

    assert_eq!(eleven_layout.columns(), 5);
    assert_eq!(eleven_layout.visible_rows(), 2);
    assert_eq!(eleven_layout.visible_item_range(), 0..11);
    assert!(eleven_layout.logical_size().1 <= 900 * 19 / 20);

    let seven_layout = SwitcherGrid::new(
        SessionDisplay::from("eDP-1"),
        (0..7).map(|index| {
            item(
                &format!("seven-{index}"),
                "com.example.Application",
                &format!("Window {index}"),
            )
        }),
        &WindowId::from("seven-1"),
    )
    .expect("the selected Window belongs to the seven-Window Grid")
    .responsive_layout(1_366, 768, CardSize::Large);

    assert_eq!(seven_layout.columns(), 5);
    assert_eq!(seven_layout.total_rows(), 2);
}

#[test]
fn fractional_scale_maps_logical_grid_geometry_to_exact_buffer_dimensions() {
    let scale = FractionalScale::from_protocol_units(150);

    assert_eq!(scale.physical_size(800, 600), (1_000, 750));
    assert_eq!(scale.ceiling_integer(), 2);
}
