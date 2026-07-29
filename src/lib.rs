// SPDX-License-Identifier: GPL-3.0-only

use std::{error::Error, fmt, ops::BitOr};

pub const APPLICATION_ID: &str = "io.github.abrahamv09.CosmicWindowSwitcher";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WindowId(String);

impl From<&str> for WindowId {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl From<String> for WindowId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl WindowId {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for WindowId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HoldModifiers(u8);

impl HoldModifiers {
    pub const ALT: Self = Self(1);
    pub const CONTROL: Self = Self(1 << 1);
    pub const SUPER: Self = Self(1 << 2);

    #[must_use]
    pub const fn empty() -> Self {
        Self(0)
    }

    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    #[must_use]
    pub const fn intersects(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }
}

impl BitOr for HoldModifiers {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SwitchingEvent {
    Tab,
    Escape,
    HoldModifiersChanged(HoldModifiers),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SessionEffect {
    None,
    SelectionChanged(WindowId),
    Activate(WindowId),
    Cancelled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SessionState {
    Open,
    Finished,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NotEnoughWindows;

impl fmt::Display for NotEnoughWindows {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a Switching Session requires at least two Windows")
    }
}

impl Error for NotEnoughWindows {}

#[derive(Clone, Debug)]
pub struct SwitchingSession {
    windows: Vec<WindowId>,
    selected: usize,
    initial_hold_modifiers: HoldModifiers,
    state: SessionState,
}

impl SwitchingSession {
    /// Starts a Switching Session in MRU Order with the previous Window selected.
    ///
    /// # Errors
    ///
    /// Returns [`NotEnoughWindows`] when fewer than two Windows are supplied.
    pub fn new(
        windows: impl IntoIterator<Item = WindowId>,
        initial_hold_modifiers: HoldModifiers,
    ) -> Result<Self, NotEnoughWindows> {
        let windows = windows.into_iter().collect::<Vec<_>>();
        if windows.len() < 2 {
            return Err(NotEnoughWindows);
        }

        Ok(Self {
            windows,
            selected: 1,
            initial_hold_modifiers,
            state: SessionState::Open,
        })
    }

    #[must_use]
    pub fn selected(&self) -> &WindowId {
        &self.windows[self.selected]
    }

    pub fn handle(&mut self, event: SwitchingEvent) -> SessionEffect {
        if self.state == SessionState::Finished {
            return SessionEffect::None;
        }

        match event {
            SwitchingEvent::Tab => {
                self.selected = (self.selected + 1) % self.windows.len();
                SessionEffect::SelectionChanged(self.selected().clone())
            }
            SwitchingEvent::Escape => {
                self.state = SessionState::Finished;
                SessionEffect::Cancelled
            }
            SwitchingEvent::HoldModifiersChanged(modifiers)
                if !self.initial_hold_modifiers.is_empty()
                    && !modifiers.intersects(self.initial_hold_modifiers) =>
            {
                self.state = SessionState::Finished;
                SessionEffect::Activate(self.selected().clone())
            }
            SwitchingEvent::HoldModifiersChanged(_) => SessionEffect::None,
        }
    }
}
