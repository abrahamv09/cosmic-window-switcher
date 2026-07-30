// SPDX-License-Identifier: GPL-3.0-only

use std::path::PathBuf;

use cosmic_config::{Config, ConfigGet, ConfigSet};
use cosmic_window_switcher::{
    APPLICATION_ID, CardSize, Dimming, PreferencesStore, RefreshCeiling, RevealDelay,
    SwitcherPreferences,
};

fn isolated_store(test_name: &str) -> (tempfile::TempDir, PreferencesStore) {
    let directory = tempfile::Builder::new()
        .prefix(test_name)
        .tempdir()
        .expect("create an isolated configuration directory");
    let config = Config::with_custom_path(
        APPLICATION_ID,
        PreferencesStore::SCHEMA_VERSION,
        PathBuf::from(directory.path()),
    )
    .expect("create an isolated cosmic-config context");
    (directory, PreferencesStore::from_config(config))
}

#[test]
fn missing_preferences_load_the_documented_defaults() {
    let (_directory, store) = isolated_store("missing-preferences");

    assert_eq!(store.load(), SwitcherPreferences::default());
    assert_eq!(store.load().card_size(), CardSize::Medium);
    assert_eq!(store.load().dimming(), Dimming::Light);
    assert_eq!(store.load().refresh_ceiling(), RefreshCeiling::Fps30);
    assert!(store.load().animations_enabled());
    assert_eq!(store.load().reveal_delay(), RevealDelay::Milliseconds100);
}

#[test]
fn saved_preferences_reload_from_cosmic_config() {
    let (_directory, store) = isolated_store("saved-preferences");
    let preferences = SwitcherPreferences::new(
        CardSize::Large,
        Dimming::Strong,
        RefreshCeiling::MatchDisplay,
        false,
        RevealDelay::Milliseconds200,
    );

    store.save(&preferences).expect("save Switcher Preferences");

    assert_eq!(store.load(), preferences);
}

#[test]
fn invalid_fields_recover_independently_without_rewriting_them() {
    let (_directory, store) = isolated_store("invalid-preferences");
    store
        .config()
        .set("card_size", "gigantic")
        .expect("write an invalid card size");
    store
        .config()
        .set("dimming", Dimming::Strong)
        .expect("write one valid preference");
    store
        .config()
        .set("refresh_ceiling", -10_i32)
        .expect("write an invalid Refresh Ceiling");
    store
        .config()
        .set("animations_enabled", "sometimes")
        .expect("write an invalid animation preference");
    store
        .config()
        .set("reveal_delay", 999_u32)
        .expect("write an invalid reveal delay");

    let loaded = store.load();

    assert_eq!(loaded.card_size(), CardSize::Medium);
    assert_eq!(loaded.dimming(), Dimming::Strong);
    assert_eq!(loaded.refresh_ceiling(), RefreshCeiling::Fps30);
    assert!(loaded.animations_enabled());
    assert_eq!(loaded.reveal_delay(), RevealDelay::Milliseconds100);
    assert_eq!(
        store
            .config()
            .get_local::<String>("card_size")
            .ok()
            .as_deref(),
        Some("gigantic")
    );
}

#[test]
fn legacy_values_migrate_in_memory_and_are_written_only_after_save() {
    let (_directory, store) = isolated_store("legacy-preferences");
    store
        .config()
        .set("thumbnail_size", "normal")
        .expect("write the legacy card-size value");
    store
        .config()
        .set("background_dimming", "off")
        .expect("write the legacy dimming value");
    store
        .config()
        .set("max_fps", 60_u32)
        .expect("write the legacy Refresh Ceiling value");
    store
        .config()
        .set("animate", false)
        .expect("write the legacy animation value");
    store
        .config()
        .set("delay_ms", 0_u32)
        .expect("write the legacy reveal-delay value");

    let migrated = store.load();

    assert_eq!(
        migrated,
        SwitcherPreferences::new(
            CardSize::Medium,
            Dimming::Off,
            RefreshCeiling::Fps60,
            false,
            RevealDelay::Immediate,
        )
    );
    assert!(store.config().get_local::<CardSize>("card_size").is_err());

    store
        .save(&migrated)
        .expect("persist the migrated preferences");

    assert_eq!(
        store.config().get_local::<CardSize>("card_size").ok(),
        Some(CardSize::Medium)
    );
}

#[test]
fn a_switching_session_keeps_the_preferences_snapshot_it_started_with() {
    let first = SwitcherPreferences::default();
    let session = first.snapshot();
    let edited = SwitcherPreferences::new(
        CardSize::Small,
        Dimming::Off,
        RefreshCeiling::Fps15,
        false,
        RevealDelay::Immediate,
    );

    assert_eq!(session.card_size(), CardSize::Medium);
    assert_eq!(session.dimming(), Dimming::Light);
    assert_eq!(session.refresh_ceiling(), RefreshCeiling::Fps30);
    assert!(session.animations_enabled());
    assert_eq!(session.reveal_delay(), RevealDelay::Milliseconds100);
    assert_ne!(session, edited.snapshot());
}
