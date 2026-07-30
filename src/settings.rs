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

const CARD_SIZE_OPTIONS: [(CardSize, StringKey); 3] = [
    (CardSize::Small, StringKey::Small),
    (CardSize::Medium, StringKey::Medium),
    (CardSize::Large, StringKey::Large),
];
const DIMMING_OPTIONS: [(Dimming, StringKey); 3] = [
    (Dimming::Off, StringKey::Off),
    (Dimming::Light, StringKey::Light),
    (Dimming::Strong, StringKey::Strong),
];
const REFRESH_CEILING_OPTIONS: [(RefreshCeiling, StringKey); 4] = [
    (RefreshCeiling::Fps15, StringKey::Fps15),
    (RefreshCeiling::Fps30, StringKey::Fps30),
    (RefreshCeiling::Fps60, StringKey::Fps60),
    (RefreshCeiling::MatchDisplay, StringKey::MatchDisplay),
];
const REVEAL_DELAY_OPTIONS: [(RevealDelay, StringKey); 3] = [
    (RevealDelay::Immediate, StringKey::Immediate),
    (RevealDelay::Milliseconds100, StringKey::Milliseconds100),
    (RevealDelay::Milliseconds200, StringKey::Milliseconds200),
];

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct ShortcutStatus {
    next: Vec<String>,
    previous: Vec<String>,
}

impl ShortcutStatus {
    fn detect() -> Self {
        let Ok(context) = shortcuts::context() else {
            return Self::default();
        };
        let configured = shortcuts::shortcuts(&context);
        Self {
            next: bindings_for(&configured, &Action::System(WindowSwitcher)),
            previous: bindings_for(&configured, &Action::System(WindowSwitcherPrevious)),
        }
    }
}

fn bindings_for(configured: &shortcuts::Shortcuts, action: &Action) -> Vec<String> {
    let mut bindings = configured
        .iter()
        .filter(|(_, candidate)| *candidate == action)
        .map(|(binding, _)| binding.to_string())
        .collect::<Vec<_>>();
    bindings.sort_unstable();
    bindings
}

fn localized_options<T>(locale: Locale, options: &[(T, StringKey)]) -> Vec<String> {
    options.iter().map(|(_, key)| locale.text(*key)).collect()
}

fn option_position<T: PartialEq>(options: &[(T, StringKey)], selected: &T) -> Option<usize> {
    options.iter().position(|(value, _)| value == selected)
}

fn option_value<T: Copy>(options: &[(T, StringKey)], index: usize, default: T) -> T {
    options.get(index).map_or(default, |(value, _)| *value)
}

fn shortcut_label(bindings: &[String], not_assigned: &str) -> String {
    if bindings.is_empty() {
        not_assigned.to_owned()
    } else {
        bindings.join(", ")
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
    CardSize(CardSize),
    Dimming(Dimming),
    RefreshCeiling(RefreshCeiling),
    Animations(bool),
    RevealDelay(RevealDelay),
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
        app.set_header_title(app.locale.text(StringKey::SettingsTitle));
        let task = app.core.main_window_id().map_or_else(Task::none, |window| {
            app.set_window_title(app.locale.text(StringKey::SettingsTitle), window)
        });
        (app, task)
    }

    fn update(&mut self, message: Self::Message) -> Task<Self::Message> {
        let next = match message {
            Message::CardSize(card_size) => self.preferences.clone().with_card_size(card_size),
            Message::Dimming(dimming) => self.preferences.clone().with_dimming(dimming),
            Message::RefreshCeiling(refresh_ceiling) => self
                .preferences
                .clone()
                .with_refresh_ceiling(refresh_ceiling),
            Message::Animations(enabled) => {
                self.preferences.clone().with_animations_enabled(enabled)
            }
            Message::RevealDelay(reveal_delay) => {
                self.preferences.clone().with_reveal_delay(reveal_delay)
            }
            Message::OpenKeyboardSettings => {
                if Command::new("cosmic-settings")
                    .arg("keyboard")
                    .spawn()
                    .is_err()
                {
                    self.save_error = Some(self.locale.text(StringKey::OpenKeyboardSettingsFailed));
                }
                return Task::none();
            }
        };

        match self.store.save(&next) {
            Ok(()) => {
                self.preferences = next;
                self.save_error = None;
            }
            Err(_) => {
                self.save_error = Some(self.locale.text(StringKey::SaveFailed));
            }
        }
        Task::none()
    }

    fn view(&self) -> Element<'_, Self::Message> {
        let text = |key| self.locale.text(key);
        let card_size = dropdown(
            localized_options(self.locale, &CARD_SIZE_OPTIONS),
            option_position(&CARD_SIZE_OPTIONS, &self.preferences.card_size()),
            |index| Message::CardSize(option_value(&CARD_SIZE_OPTIONS, index, CardSize::Medium)),
        );
        let dimming = dropdown(
            localized_options(self.locale, &DIMMING_OPTIONS),
            option_position(&DIMMING_OPTIONS, &self.preferences.dimming()),
            |index| Message::Dimming(option_value(&DIMMING_OPTIONS, index, Dimming::Light)),
        );
        let refresh_ceiling = dropdown(
            localized_options(self.locale, &REFRESH_CEILING_OPTIONS),
            option_position(
                &REFRESH_CEILING_OPTIONS,
                &self.preferences.refresh_ceiling(),
            ),
            |index| {
                Message::RefreshCeiling(option_value(
                    &REFRESH_CEILING_OPTIONS,
                    index,
                    RefreshCeiling::Fps30,
                ))
            },
        );
        let reveal_delay = dropdown(
            localized_options(self.locale, &REVEAL_DELAY_OPTIONS),
            option_position(&REVEAL_DELAY_OPTIONS, &self.preferences.reveal_delay()),
            |index| {
                Message::RevealDelay(option_value(
                    &REVEAL_DELAY_OPTIONS,
                    index,
                    RevealDelay::Milliseconds100,
                ))
            },
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
        let next = shortcut_label(&self.shortcuts.next, &not_assigned);
        let previous = shortcut_label(&self.shortcuts.previous, &not_assigned);
        let shortcuts = settings::section()
            .title(text(StringKey::Shortcuts))
            .add(settings::item::builder(text(StringKey::NextWindow)).control(widget::text(next)))
            .add(
                settings::item::builder(text(StringKey::PreviousWindow))
                    .control(widget::text(previous)),
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
