// SPDX-License-Identifier: GPL-3.0-only

use std::process::Command;

use anyhow::{Context, Result};
use cosmic::{
    ApplicationExt, Element,
    app::{Core, Settings, Task},
    executor,
    iced::{Length, Size},
    widget::{self, button, column, dropdown, settings},
};
use cosmic_settings_config::shortcuts::{
    self, Action,
    action::System::{WindowSwitcher, WindowSwitcherPrevious},
};
use cosmic_window_switcher::{
    APPLICATION_ID, CardSize, Dimming, Locale, PreferencesStore, RefreshCeiling, RevealDelay,
    StringKey, SwitcherPreferences,
};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct ShortcutStatus {
    next: Option<String>,
    previous: Option<String>,
}

impl ShortcutStatus {
    fn detect() -> Self {
        let Ok(context) = shortcuts::context() else {
            return Self::default();
        };
        let configured = shortcuts::shortcuts(&context);
        Self {
            next: configured.shortcut_for_action(&Action::System(WindowSwitcher)),
            previous: configured.shortcut_for_action(&Action::System(WindowSwitcherPrevious)),
        }
    }
}

pub(super) fn run() -> Result<()> {
    let store = PreferencesStore::open().context("open Switcher Preferences")?;
    let flags = Flags {
        preferences: store.load(),
        store,
        locale: Locale::detect(),
        shortcuts: ShortcutStatus::detect(),
    };
    let settings = Settings::default()
        .client_decorations(true)
        .size(Size::new(720.0, 680.0));
    cosmic::app::run::<SettingsApp>(settings, flags).context("run the native settings window")
}

struct Flags {
    preferences: SwitcherPreferences,
    store: PreferencesStore,
    locale: Locale,
    shortcuts: ShortcutStatus,
}

struct SettingsApp {
    core: Core,
    preferences: SwitcherPreferences,
    store: PreferencesStore,
    locale: Locale,
    shortcuts: ShortcutStatus,
    save_error: Option<String>,
}

#[derive(Clone, Debug)]
enum Message {
    CardSize(usize),
    Dimming(usize),
    RefreshCeiling(usize),
    Animations(bool),
    RevealDelay(usize),
    OpenKeyboardSettings,
}

impl cosmic::Application for SettingsApp {
    type Executor = executor::Default;
    type Flags = Flags;
    type Message = Message;

    const APP_ID: &'static str = APPLICATION_ID;

    fn core(&self) -> &Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    fn init(core: Core, flags: Self::Flags) -> (Self, Task<Self::Message>) {
        let mut app = Self {
            core,
            preferences: flags.preferences,
            store: flags.store,
            locale: flags.locale,
            shortcuts: flags.shortcuts,
            save_error: None,
        };
        app.set_header_title(app.locale.text(StringKey::SettingsTitle).to_owned());
        let task = app.core.main_window_id().map_or_else(Task::none, |window| {
            app.set_window_title(app.locale.text(StringKey::SettingsTitle).to_owned(), window)
        });
        (app, task)
    }

    fn update(&mut self, message: Self::Message) -> Task<Self::Message> {
        let next = match message {
            Message::CardSize(index) => SwitcherPreferences::new(
                [CardSize::Small, CardSize::Medium, CardSize::Large]
                    .get(index)
                    .copied()
                    .unwrap_or(self.preferences.card_size()),
                self.preferences.dimming(),
                self.preferences.refresh_ceiling(),
                self.preferences.animations_enabled(),
                self.preferences.reveal_delay(),
            ),
            Message::Dimming(index) => SwitcherPreferences::new(
                self.preferences.card_size(),
                [Dimming::Off, Dimming::Light, Dimming::Strong]
                    .get(index)
                    .copied()
                    .unwrap_or(self.preferences.dimming()),
                self.preferences.refresh_ceiling(),
                self.preferences.animations_enabled(),
                self.preferences.reveal_delay(),
            ),
            Message::RefreshCeiling(index) => SwitcherPreferences::new(
                self.preferences.card_size(),
                self.preferences.dimming(),
                [
                    RefreshCeiling::Fps15,
                    RefreshCeiling::Fps30,
                    RefreshCeiling::Fps60,
                    RefreshCeiling::MatchDisplay,
                ]
                .get(index)
                .copied()
                .unwrap_or(self.preferences.refresh_ceiling()),
                self.preferences.animations_enabled(),
                self.preferences.reveal_delay(),
            ),
            Message::Animations(enabled) => SwitcherPreferences::new(
                self.preferences.card_size(),
                self.preferences.dimming(),
                self.preferences.refresh_ceiling(),
                enabled,
                self.preferences.reveal_delay(),
            ),
            Message::RevealDelay(index) => SwitcherPreferences::new(
                self.preferences.card_size(),
                self.preferences.dimming(),
                self.preferences.refresh_ceiling(),
                self.preferences.animations_enabled(),
                [
                    RevealDelay::Immediate,
                    RevealDelay::Milliseconds100,
                    RevealDelay::Milliseconds200,
                ]
                .get(index)
                .copied()
                .unwrap_or(self.preferences.reveal_delay()),
            ),
            Message::OpenKeyboardSettings => {
                if let Err(error) = Command::new("cosmic-settings").arg("keyboard").spawn() {
                    self.save_error = Some(error.to_string());
                }
                return Task::none();
            }
        };

        match self.store.save(&next) {
            Ok(()) => {
                self.preferences = next;
                self.save_error = None;
            }
            Err(error) => self.save_error = Some(error.to_string()),
        }
        Task::none()
    }

    fn view(&self) -> Element<'_, Self::Message> {
        let text = |key| self.locale.text(key);
        let card_size = dropdown(
            vec![
                text(StringKey::Small).to_owned(),
                text(StringKey::Medium).to_owned(),
                text(StringKey::Large).to_owned(),
            ],
            Some(match self.preferences.card_size() {
                CardSize::Small => 0,
                CardSize::Medium => 1,
                CardSize::Large => 2,
            }),
            Message::CardSize,
        );
        let dimming = dropdown(
            vec![
                text(StringKey::Off).to_owned(),
                text(StringKey::Light).to_owned(),
                text(StringKey::Strong).to_owned(),
            ],
            Some(match self.preferences.dimming() {
                Dimming::Off => 0,
                Dimming::Light => 1,
                Dimming::Strong => 2,
            }),
            Message::Dimming,
        );
        let refresh_ceiling = dropdown(
            vec![
                text(StringKey::Fps15).to_owned(),
                text(StringKey::Fps30).to_owned(),
                text(StringKey::Fps60).to_owned(),
                text(StringKey::MatchDisplay).to_owned(),
            ],
            Some(match self.preferences.refresh_ceiling() {
                RefreshCeiling::Fps15 => 0,
                RefreshCeiling::Fps30 => 1,
                RefreshCeiling::Fps60 => 2,
                RefreshCeiling::MatchDisplay => 3,
            }),
            Message::RefreshCeiling,
        );
        let reveal_delay = dropdown(
            vec![
                text(StringKey::Immediate).to_owned(),
                text(StringKey::Milliseconds100).to_owned(),
                text(StringKey::Milliseconds200).to_owned(),
            ],
            Some(match self.preferences.reveal_delay() {
                RevealDelay::Immediate => 0,
                RevealDelay::Milliseconds100 => 1,
                RevealDelay::Milliseconds200 => 2,
            }),
            Message::RevealDelay,
        );

        let preferences = settings::section()
            .title(text(StringKey::WindowSwitcher))
            .add(settings::item::builder(text(StringKey::CardSize)).control(card_size))
            .add(settings::item::builder(text(StringKey::BackgroundDimming)).control(dimming))
            .add(
                settings::item::builder(text(StringKey::RefreshCeiling))
                    .description(text(StringKey::MatchDisplayWarning))
                    .control(refresh_ceiling),
            )
            .add(
                settings::item::builder(text(StringKey::Animations))
                    .toggler(self.preferences.animations_enabled(), Message::Animations),
            )
            .add(settings::item::builder(text(StringKey::RevealDelay)).control(reveal_delay));

        let not_assigned = text(StringKey::NotAssigned);
        let shortcuts = settings::section()
            .title(text(StringKey::Shortcuts))
            .add(
                settings::item::builder(text(StringKey::NextWindow)).control(widget::text(
                    self.shortcuts.next.as_deref().unwrap_or(not_assigned),
                )),
            )
            .add(
                settings::item::builder(text(StringKey::PreviousWindow)).control(widget::text(
                    self.shortcuts.previous.as_deref().unwrap_or(not_assigned),
                )),
            )
            .add(
                settings::item::builder(text(StringKey::ShortcutInstructions)).control(
                    button::standard(text(StringKey::OpenKeyboardSettings))
                        .on_press(Message::OpenKeyboardSettings),
                ),
            );

        let mut content = column::with_children(vec![
            preferences.into(),
            widget::text::body(text(StringKey::SavedForNextSession)).into(),
            shortcuts.into(),
        ])
        .spacing(24)
        .padding(24)
        .width(Length::Fill);
        if let Some(error) = &self.save_error {
            content = content.push(widget::text::body(error));
        }
        content.into()
    }
}
