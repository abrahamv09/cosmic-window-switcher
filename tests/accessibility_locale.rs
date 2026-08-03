// SPDX-License-Identifier: GPL-3.0-only

use cosmic_window_switcher::{
    AccessibilityPolicy, CardSize, Dimming, Locale, OverlayPresentation, RefreshCeiling,
    RevealDelay, SessionDisplay, StringKey, SwitcherGrid, SwitcherItem, SwitcherPreferences,
    WindowId,
};
use std::time::Duration;

fn item(id: &str, application_id: &str, title: &str) -> SwitcherItem {
    SwitcherItem::new(
        WindowId::from(id),
        application_id.to_owned(),
        title.to_owned(),
    )
}

#[test]
fn locale_follows_english_and_spanish_desktop_language_tags() {
    assert_eq!(Locale::from_language_tag("en_US.UTF-8"), Locale::English);
    assert_eq!(Locale::from_language_tag("C"), Locale::English);
    assert_eq!(Locale::from_language_tag("es_ES.UTF-8"), Locale::Spanish);
    assert_eq!(Locale::from_language_tag("es-419"), Locale::Spanish);
    assert_eq!(Locale::from_language_tag("fr:es"), Locale::Spanish);
    assert_eq!(Locale::from_language_tag(":es"), Locale::Spanish);
}

#[test]
fn every_user_facing_string_is_complete_in_english_and_spanish() {
    for key in StringKey::ALL {
        let mut arguments = fluent_bundle::FluentArgs::new();
        arguments.set("advertised", "v1");
        arguments.set("required", 2_u32);
        let parameterized = matches!(
            key,
            StringKey::ToplevelInfoFailure
                | StringKey::WorkspaceProtocolFailure
                | StringKey::WorkspaceSnapshotFailure
                | StringKey::ToplevelMembershipFailure
        );
        let arguments = parameterized.then_some(&arguments);
        let english = Locale::English.format(key, arguments);
        let spanish = Locale::Spanish.format(key, arguments);
        assert!(
            !english.trim().is_empty(),
            "missing English text for {key:?}"
        );
        assert!(
            !spanish.trim().is_empty(),
            "missing Spanish text for {key:?}"
        );
    }
    assert_ne!(
        Locale::English.text(StringKey::SettingsTitle),
        Locale::Spanish.text(StringKey::SettingsTitle)
    );
}

#[test]
fn switcher_items_expose_name_selection_position_and_localized_instructions() {
    let grid = SwitcherGrid::new(
        SessionDisplay::from("eDP-1"),
        [
            item("focused", "com.example.Editor", "Notas"),
            item("selected", "com.example.Terminal", "Compilar"),
        ],
        &WindowId::from("selected"),
    )
    .expect("the selected Window belongs to the grid");

    let first = grid.items()[0].accessibility(Locale::Spanish);
    let selected = grid.items()[1].accessibility(Locale::Spanish);

    assert_eq!(first.name(), "Notas");
    assert!(!first.is_selected());
    assert_eq!(first.position(), 1);
    assert_eq!(first.set_size(), 2);
    assert!(first.instructions().contains("Tab"));
    assert_eq!(selected.name(), "Compilar");
    assert!(selected.is_selected());
    assert_eq!(selected.position(), 2);
    assert_eq!(selected.set_size(), 2);
}

#[test]
fn an_untitled_window_uses_a_localized_accessible_fallback() {
    let item = SwitcherItem::new_localized(
        WindowId::from("untitled"),
        String::new(),
        String::new(),
        Locale::Spanish,
    );

    assert_eq!(item.title(), "Ventana sin título");
    assert_eq!(
        item.accessibility(Locale::Spanish).name(),
        "Ventana sin título"
    );
}

#[test]
fn accessibility_policy_overrides_presentation_without_mutating_preferences() {
    let preferences = SwitcherPreferences::new(
        CardSize::Large,
        Dimming::Strong,
        RefreshCeiling::Fps60,
        true,
        false,
        RevealDelay::Milliseconds200,
    );
    let session = preferences.snapshot();

    let presentation = OverlayPresentation::resolve(&session, AccessibilityPolicy::new(true, true));

    assert!(presentation.high_contrast());
    assert!(!presentation.animations_enabled());
    assert_eq!(presentation.dimming(), Dimming::Strong);
    assert_eq!(session.dimming(), Dimming::Strong);
    assert!(session.animations_enabled());
}

#[test]
fn animation_preference_drives_a_short_reveal_fade_and_reduced_motion_skips_it() {
    let session = SwitcherPreferences::default().snapshot();
    let animated = OverlayPresentation::resolve(&session, AccessibilityPolicy::new(false, false));
    let reduced = OverlayPresentation::resolve(&session, AccessibilityPolicy::new(false, true));

    assert_eq!(animated.reveal_opacity(Duration::ZERO), 0);
    assert!(animated.reveal_opacity(Duration::from_millis(75)) > 0);
    assert_eq!(animated.reveal_opacity(Duration::from_millis(150)), u8::MAX);
    assert_eq!(reduced.reveal_opacity(Duration::ZERO), u8::MAX);
}
