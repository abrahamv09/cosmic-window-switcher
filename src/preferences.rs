// SPDX-License-Identifier: GPL-3.0-only

use std::time::Duration;

use cosmic_config::{Config, ConfigGet, ConfigSet};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::{APPLICATION_ID, CardSize, RefreshCeiling};

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
pub enum Dimming {
    Off,
    #[default]
    Light,
    Strong,
}

impl Dimming {
    #[must_use]
    pub const fn alpha(self) -> u8 {
        match self {
            Self::Off => 0,
            Self::Light => 120,
            Self::Strong => 210,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
pub enum RevealDelay {
    Immediate,
    #[default]
    Milliseconds100,
    Milliseconds200,
}

impl RevealDelay {
    #[must_use]
    pub const fn duration(self) -> Duration {
        match self {
            Self::Immediate => Duration::ZERO,
            Self::Milliseconds100 => Duration::from_millis(100),
            Self::Milliseconds200 => Duration::from_millis(200),
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct SwitcherPreferences {
    card_size: CardSize,
    dimming: Dimming,
    refresh_ceiling: RefreshCeiling,
    animations_enabled: bool,
    reveal_delay: RevealDelay,
}

impl Default for SwitcherPreferences {
    fn default() -> Self {
        Self {
            card_size: CardSize::Medium,
            dimming: Dimming::Light,
            refresh_ceiling: RefreshCeiling::Fps30,
            animations_enabled: true,
            reveal_delay: RevealDelay::Milliseconds100,
        }
    }
}

impl SwitcherPreferences {
    #[must_use]
    pub const fn new(
        card_size: CardSize,
        dimming: Dimming,
        refresh_ceiling: RefreshCeiling,
        animations_enabled: bool,
        reveal_delay: RevealDelay,
    ) -> Self {
        Self {
            card_size,
            dimming,
            refresh_ceiling,
            animations_enabled,
            reveal_delay,
        }
    }

    #[must_use]
    pub const fn card_size(&self) -> CardSize {
        self.card_size
    }

    #[must_use]
    pub const fn dimming(&self) -> Dimming {
        self.dimming
    }

    #[must_use]
    pub const fn refresh_ceiling(&self) -> RefreshCeiling {
        self.refresh_ceiling
    }

    #[must_use]
    pub const fn animations_enabled(&self) -> bool {
        self.animations_enabled
    }

    #[must_use]
    pub const fn reveal_delay(&self) -> RevealDelay {
        self.reveal_delay
    }

    #[must_use]
    pub fn snapshot(&self) -> SessionPreferences {
        SessionPreferences(self.clone())
    }

    #[must_use]
    pub const fn with_card_size(mut self, card_size: CardSize) -> Self {
        self.card_size = card_size;
        self
    }

    #[must_use]
    pub const fn with_dimming(mut self, dimming: Dimming) -> Self {
        self.dimming = dimming;
        self
    }

    #[must_use]
    pub const fn with_refresh_ceiling(mut self, refresh_ceiling: RefreshCeiling) -> Self {
        self.refresh_ceiling = refresh_ceiling;
        self
    }

    #[must_use]
    pub const fn with_animations_enabled(mut self, animations_enabled: bool) -> Self {
        self.animations_enabled = animations_enabled;
        self
    }

    #[must_use]
    pub const fn with_reveal_delay(mut self, reveal_delay: RevealDelay) -> Self {
        self.reveal_delay = reveal_delay;
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionPreferences(SwitcherPreferences);

impl SessionPreferences {
    #[must_use]
    pub const fn card_size(&self) -> CardSize {
        self.0.card_size()
    }

    #[must_use]
    pub const fn dimming(&self) -> Dimming {
        self.0.dimming()
    }

    #[must_use]
    pub const fn refresh_ceiling(&self) -> RefreshCeiling {
        self.0.refresh_ceiling()
    }

    #[must_use]
    pub const fn animations_enabled(&self) -> bool {
        self.0.animations_enabled()
    }

    #[must_use]
    pub const fn reveal_delay(&self) -> RevealDelay {
        self.0.reveal_delay()
    }
}

#[derive(Clone, Debug)]
pub struct PreferencesStore {
    config: Config,
}

impl PreferencesStore {
    pub const SCHEMA_VERSION: u64 = 1;

    /// Opens the app-owned `cosmic-config` namespace.
    ///
    /// # Errors
    ///
    /// Returns an error when the user's COSMIC configuration directory cannot
    /// be opened.
    pub fn open() -> Result<Self, cosmic_config::Error> {
        Config::new(APPLICATION_ID, Self::SCHEMA_VERSION).map(Self::from_config)
    }

    #[must_use]
    pub const fn from_config(config: Config) -> Self {
        Self { config }
    }

    #[must_use]
    pub const fn config(&self) -> &Config {
        &self.config
    }

    #[must_use]
    pub fn load(&self) -> SwitcherPreferences {
        let defaults = SwitcherPreferences::default();
        SwitcherPreferences::new(
            self.read("card_size")
                .or_else(|| self.legacy_card_size())
                .unwrap_or(defaults.card_size()),
            self.read("dimming")
                .or_else(|| self.legacy_dimming())
                .unwrap_or(defaults.dimming()),
            self.read("refresh_ceiling")
                .or_else(|| self.legacy_refresh_ceiling())
                .unwrap_or(defaults.refresh_ceiling()),
            self.read("animations_enabled")
                .or_else(|| self.read("animate"))
                .unwrap_or(defaults.animations_enabled()),
            self.read("reveal_delay")
                .or_else(|| self.legacy_reveal_delay())
                .unwrap_or(defaults.reveal_delay()),
        )
    }

    /// Atomically persists all app-owned Switcher Preferences.
    ///
    /// # Errors
    ///
    /// Returns an error when any value cannot be serialized or committed.
    pub fn save(&self, preferences: &SwitcherPreferences) -> Result<(), cosmic_config::Error> {
        let transaction = self.config.transaction();
        transaction.set("card_size", preferences.card_size())?;
        transaction.set("dimming", preferences.dimming())?;
        transaction.set("refresh_ceiling", preferences.refresh_ceiling())?;
        transaction.set("animations_enabled", preferences.animations_enabled())?;
        transaction.set("reveal_delay", preferences.reveal_delay())?;
        transaction.commit()
    }

    fn read<T: DeserializeOwned>(&self, key: &str) -> Option<T> {
        self.config.get(key).ok()
    }

    fn legacy_card_size(&self) -> Option<CardSize> {
        match self
            .read::<String>("thumbnail_size")?
            .to_ascii_lowercase()
            .as_str()
        {
            "small" => Some(CardSize::Small),
            "normal" | "medium" => Some(CardSize::Medium),
            "large" => Some(CardSize::Large),
            _ => None,
        }
    }

    fn legacy_dimming(&self) -> Option<Dimming> {
        match self
            .read::<String>("background_dimming")?
            .to_ascii_lowercase()
            .as_str()
        {
            "off" => Some(Dimming::Off),
            "light" => Some(Dimming::Light),
            "strong" => Some(Dimming::Strong),
            _ => None,
        }
    }

    fn legacy_refresh_ceiling(&self) -> Option<RefreshCeiling> {
        match self.read::<u32>("max_fps")? {
            15 => Some(RefreshCeiling::Fps15),
            30 => Some(RefreshCeiling::Fps30),
            60 => Some(RefreshCeiling::Fps60),
            0 => Some(RefreshCeiling::MatchDisplay),
            _ => None,
        }
    }

    fn legacy_reveal_delay(&self) -> Option<RevealDelay> {
        match self.read::<u32>("delay_ms")? {
            0 => Some(RevealDelay::Immediate),
            100 => Some(RevealDelay::Milliseconds100),
            200 => Some(RevealDelay::Milliseconds200),
            _ => None,
        }
    }
}
