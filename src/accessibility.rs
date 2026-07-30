// SPDX-License-Identifier: GPL-3.0-only

use crate::{Dimming, Locale, SessionPreferences, StringKey};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AccessibilityPolicy {
    screen_reader_active: bool,
    high_contrast: bool,
    reduced_motion: bool,
}

impl AccessibilityPolicy {
    #[must_use]
    pub const fn new(
        screen_reader_active: bool,
        high_contrast: bool,
        reduced_motion: bool,
    ) -> Self {
        Self {
            screen_reader_active,
            high_contrast,
            reduced_motion,
        }
    }

    #[must_use]
    pub const fn screen_reader_active(self) -> bool {
        self.screen_reader_active
    }

    #[must_use]
    pub const fn high_contrast(self) -> bool {
        self.high_contrast
    }

    #[must_use]
    pub const fn reduced_motion(self) -> bool {
        self.reduced_motion
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OverlayPresentation {
    dimming: Dimming,
    animations_enabled: bool,
    high_contrast: bool,
}

impl OverlayPresentation {
    #[must_use]
    pub const fn resolve(
        preferences: &SessionPreferences,
        accessibility: AccessibilityPolicy,
    ) -> Self {
        Self {
            dimming: preferences.dimming(),
            animations_enabled: preferences.animations_enabled() && !accessibility.reduced_motion(),
            high_contrast: accessibility.high_contrast(),
        }
    }

    #[must_use]
    pub const fn dimming(self) -> Dimming {
        self.dimming
    }

    #[must_use]
    pub const fn animations_enabled(self) -> bool {
        self.animations_enabled
    }

    #[must_use]
    pub const fn high_contrast(self) -> bool {
        self.high_contrast
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccessibleSwitcherItem<'a> {
    name: &'a str,
    selected: bool,
    position: usize,
    set_size: usize,
    instructions: &'static str,
}

impl<'a> AccessibleSwitcherItem<'a> {
    #[must_use]
    pub const fn new(
        name: &'a str,
        selected: bool,
        position: usize,
        set_size: usize,
        locale: Locale,
    ) -> Self {
        Self {
            name,
            selected,
            position,
            set_size,
            instructions: locale.text(StringKey::InteractionInstructions),
        }
    }

    #[must_use]
    pub const fn name(&self) -> &str {
        self.name
    }

    #[must_use]
    pub const fn is_selected(&self) -> bool {
        self.selected
    }

    #[must_use]
    pub const fn position(&self) -> usize {
        self.position
    }

    #[must_use]
    pub const fn set_size(&self) -> usize {
        self.set_size
    }

    #[must_use]
    pub const fn instructions(&self) -> &'static str {
        self.instructions
    }
}
