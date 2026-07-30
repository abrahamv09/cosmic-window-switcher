// SPDX-License-Identifier: GPL-3.0-only

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Locale {
    #[default]
    English,
    Spanish,
}

impl Locale {
    #[must_use]
    pub fn detect() -> Self {
        ["LANGUAGE", "LC_ALL", "LC_MESSAGES", "LANG"]
            .into_iter()
            .find_map(|name| std::env::var(name).ok())
            .map_or(Self::English, |language| Self::from_language_tag(&language))
    }

    #[must_use]
    pub fn from_language_tag(language: &str) -> Self {
        let language = language
            .split([':', '.', '@'])
            .next()
            .unwrap_or(language)
            .replace('_', "-")
            .to_ascii_lowercase();
        if language == "es" || language.starts_with("es-") {
            Self::Spanish
        } else {
            Self::English
        }
    }

    #[must_use]
    pub const fn text(self, key: StringKey) -> &'static str {
        match (self, key) {
            (Self::English, StringKey::SettingsTitle) => "COSMIC Window Switcher Settings",
            (Self::Spanish, StringKey::SettingsTitle) => {
                "Ajustes del selector de ventanas de COSMIC"
            }
            (Self::English, StringKey::WindowSwitcher) => "COSMIC Window Switcher",
            (Self::Spanish, StringKey::WindowSwitcher) => "Selector de ventanas de COSMIC",
            (Self::English, StringKey::CardSize) => "Card size",
            (Self::Spanish, StringKey::CardSize) => "Tamaño de tarjeta",
            (Self::English, StringKey::Small) => "Small",
            (Self::Spanish, StringKey::Small) => "Pequeña",
            (Self::English, StringKey::Medium) => "Medium",
            (Self::Spanish, StringKey::Medium) => "Mediana",
            (Self::English, StringKey::Large) => "Large",
            (Self::Spanish, StringKey::Large) => "Grande",
            (Self::English, StringKey::BackgroundDimming) => "Background dimming",
            (Self::Spanish, StringKey::BackgroundDimming) => "Oscurecimiento del fondo",
            (Self::English, StringKey::Off) => "Off",
            (Self::Spanish, StringKey::Off) => "Desactivado",
            (Self::English, StringKey::Light) => "Light",
            (Self::Spanish, StringKey::Light) => "Ligero",
            (Self::English, StringKey::Strong) => "Strong",
            (Self::Spanish, StringKey::Strong) => "Intenso",
            (Self::English, StringKey::RefreshCeiling) => "Refresh Ceiling",
            (Self::Spanish, StringKey::RefreshCeiling) => "Límite de actualización",
            (Self::English | Self::Spanish, StringKey::Fps15) => "15 FPS",
            (Self::English | Self::Spanish, StringKey::Fps30) => "30 FPS",
            (Self::English | Self::Spanish, StringKey::Fps60) => "60 FPS",
            (Self::English, StringKey::MatchDisplay) => "Match display",
            (Self::Spanish, StringKey::MatchDisplay) => "Igualar la pantalla",
            (Self::English, StringKey::MatchDisplayWarning) => {
                "May use substantially more power and graphics resources."
            }
            (Self::Spanish, StringKey::MatchDisplayWarning) => {
                "Puede usar mucha más energía y recursos gráficos."
            }
            (Self::English, StringKey::Animations) => "Animations",
            (Self::Spanish, StringKey::Animations) => "Animaciones",
            (Self::English, StringKey::RevealDelay) => "Reveal delay",
            (Self::Spanish, StringKey::RevealDelay) => "Retraso de aparición",
            (Self::English, StringKey::Immediate) => "Immediate",
            (Self::Spanish, StringKey::Immediate) => "Inmediato",
            (Self::English | Self::Spanish, StringKey::Milliseconds100) => "100 ms",
            (Self::English | Self::Spanish, StringKey::Milliseconds200) => "200 ms",
            (Self::English, StringKey::Shortcuts) => "Keyboard shortcuts",
            (Self::Spanish, StringKey::Shortcuts) => "Atajos de teclado",
            (Self::English, StringKey::NextWindow) => "Next Window",
            (Self::Spanish, StringKey::NextWindow) => "Ventana siguiente",
            (Self::English, StringKey::PreviousWindow) => "Previous Window",
            (Self::Spanish, StringKey::PreviousWindow) => "Ventana anterior",
            (Self::English, StringKey::NotAssigned) => "Not assigned",
            (Self::Spanish, StringKey::NotAssigned) => "Sin asignar",
            (Self::English, StringKey::OpenKeyboardSettings) => "Open COSMIC Keyboard Settings",
            (Self::Spanish, StringKey::OpenKeyboardSettings) => {
                "Abrir los ajustes de teclado de COSMIC"
            }
            (Self::English, StringKey::ShortcutInstructions) => {
                "COSMIC owns shortcut assignment. Change it in Keyboard Settings."
            }
            (Self::Spanish, StringKey::ShortcutInstructions) => {
                "COSMIC controla los atajos. Cámbielos en los ajustes de teclado."
            }
            (Self::English, StringKey::InteractionInstructions) => {
                "Use Tab or Shift+Tab to move, Enter to select, and Escape to cancel."
            }
            (Self::Spanish, StringKey::InteractionInstructions) => {
                "Use Tab o Mayús+Tab para moverse, Intro para elegir y Escape para cancelar."
            }
            (Self::English, StringKey::SavedForNextSession) => {
                "Saved changes apply the next time the switcher opens."
            }
            (Self::Spanish, StringKey::SavedForNextSession) => {
                "Los cambios guardados se aplican la próxima vez que se abra el selector."
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StringKey {
    SettingsTitle,
    WindowSwitcher,
    CardSize,
    Small,
    Medium,
    Large,
    BackgroundDimming,
    Off,
    Light,
    Strong,
    RefreshCeiling,
    Fps15,
    Fps30,
    Fps60,
    MatchDisplay,
    MatchDisplayWarning,
    Animations,
    RevealDelay,
    Immediate,
    Milliseconds100,
    Milliseconds200,
    Shortcuts,
    NextWindow,
    PreviousWindow,
    NotAssigned,
    OpenKeyboardSettings,
    ShortcutInstructions,
    InteractionInstructions,
    SavedForNextSession,
}

impl StringKey {
    pub const ALL: [Self; 29] = [
        Self::SettingsTitle,
        Self::WindowSwitcher,
        Self::CardSize,
        Self::Small,
        Self::Medium,
        Self::Large,
        Self::BackgroundDimming,
        Self::Off,
        Self::Light,
        Self::Strong,
        Self::RefreshCeiling,
        Self::Fps15,
        Self::Fps30,
        Self::Fps60,
        Self::MatchDisplay,
        Self::MatchDisplayWarning,
        Self::Animations,
        Self::RevealDelay,
        Self::Immediate,
        Self::Milliseconds100,
        Self::Milliseconds200,
        Self::Shortcuts,
        Self::NextWindow,
        Self::PreviousWindow,
        Self::NotAssigned,
        Self::OpenKeyboardSettings,
        Self::ShortcutInstructions,
        Self::InteractionInstructions,
        Self::SavedForNextSession,
    ];
}
