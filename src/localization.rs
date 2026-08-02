// SPDX-License-Identifier: GPL-3.0-only

use std::sync::LazyLock;

use fluent_bundle::{FluentArgs, FluentResource, concurrent::FluentBundle};
use unic_langid::LanguageIdentifier;

type Bundle = FluentBundle<FluentResource>;

static ENGLISH: LazyLock<Bundle> = LazyLock::new(|| {
    bundle(
        "en-US",
        include_str!("../i18n/en/cosmic-window-switcher.ftl"),
    )
});
static SPANISH: LazyLock<Bundle> =
    LazyLock::new(|| bundle("es", include_str!("../i18n/es/cosmic-window-switcher.ftl")));

fn bundle(language: &str, source: &str) -> Bundle {
    let language = language
        .parse::<LanguageIdentifier>()
        .expect("built-in locale is valid");
    let resource = FluentResource::try_new(source.to_owned())
        .expect("built-in Fluent resource contains no syntax errors");
    let mut bundle = FluentBundle::new_concurrent(vec![language]);
    bundle.set_use_isolating(false);
    bundle
        .add_resource(resource)
        .expect("built-in Fluent resource has unique message identifiers");
    bundle
}

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
            .filter_map(|name| std::env::var(name).ok())
            .find_map(|language| Self::supported_from_language_list(&language))
            .unwrap_or(Self::English)
    }

    #[must_use]
    pub fn from_language_tag(language: &str) -> Self {
        Self::supported_from_language_list(language).unwrap_or(Self::English)
    }

    fn supported_from_language_list(language: &str) -> Option<Self> {
        language.split(':').find_map(|language| {
            let language = language
                .split(['.', '@'])
                .next()
                .unwrap_or(language)
                .replace('_', "-")
                .to_ascii_lowercase();
            if language == "es" || language.starts_with("es-") {
                Some(Self::Spanish)
            } else if language == "en"
                || language.starts_with("en-")
                || language == "c"
                || language == "posix"
            {
                Some(Self::English)
            } else {
                None
            }
        })
    }

    #[must_use]
    pub fn text(self, key: StringKey) -> String {
        self.format(key, None)
    }

    #[must_use]
    /// Formats a built-in Fluent message.
    ///
    /// # Panics
    ///
    /// Panics when a built-in resource is missing the requested message or
    /// contains a formatting error. Both bundled locales are validated by tests.
    pub fn format(self, key: StringKey, arguments: Option<&FluentArgs<'_>>) -> String {
        let bundle = match self {
            Self::English => &*ENGLISH,
            Self::Spanish => &*SPANISH,
        };
        let message = bundle
            .get_message(key.id())
            .unwrap_or_else(|| panic!("built-in locale is missing {}", key.id()));
        let pattern = message
            .value()
            .unwrap_or_else(|| panic!("built-in locale message {} has no value", key.id()));
        let mut errors = Vec::new();
        let value = bundle
            .format_pattern(pattern, arguments, &mut errors)
            .into_owned();
        assert!(
            errors.is_empty(),
            "built-in locale message {} failed to format: {errors:?}",
            key.id()
        );
        value
    }
}

macro_rules! string_keys {
    ($($key:ident => $id:literal),+ $(,)?) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum StringKey {
            $($key),+
        }

        impl StringKey {
            pub const ALL: [Self; string_keys!(@count $($key),+)] = [
                $(Self::$key),+
            ];

            const fn id(self) -> &'static str {
                match self {
                    $(Self::$key => $id),+
                }
            }
        }
    };
    (@count $($key:ident),+) => {
        <[()]>::len(&[$(string_keys!(@unit $key)),+])
    };
    (@unit $key:ident) => { () };
}

string_keys!(
    SettingsTitle => "settings-title",
    WindowSwitcher => "window-switcher",
    CardSize => "card-size",
    Small => "small",
    Medium => "medium",
    Large => "large",
    BackgroundDimming => "background-dimming",
    Off => "off",
    Light => "light",
    Strong => "strong",
    RefreshCeiling => "refresh-ceiling",
    Fps15 => "fps-15",
    Fps30 => "fps-30",
    Fps60 => "fps-60",
    MatchDisplay => "match-display",
    MatchDisplayWarning => "match-display-warning",
    Animations => "animations",
    RevealDelay => "reveal-delay",
    Milliseconds20 => "milliseconds-20",
    Milliseconds40 => "milliseconds-40",
    Milliseconds60 => "milliseconds-60",
    Milliseconds80 => "milliseconds-80",
    Milliseconds100 => "milliseconds-100",
    Milliseconds200 => "milliseconds-200",
    Shortcuts => "shortcuts",
    NextWindow => "next-window",
    PreviousWindow => "previous-window",
    NotAssigned => "not-assigned",
    OpenKeyboardSettings => "open-keyboard-settings",
    ShortcutInstructions => "shortcut-instructions",
    InteractionInstructions => "interaction-instructions",
    SavedForNextSession => "saved-for-next-session",
    SaveFailed => "save-failed",
    OpenKeyboardSettingsFailed => "open-keyboard-settings-failed",
    UntitledWindow => "untitled-window",
    CliAbout => "cli-about",
    CliService => "cli-service",
    CliEnable => "cli-enable",
    CliDisable => "cli-disable",
    CliStatus => "cli-status",
    CliDoctor => "cli-doctor",
    CliSettings => "cli-settings",
    CliInvoke => "cli-invoke",
    CliNext => "cli-next",
    CliPrevious => "cli-previous",
    CliProbe => "cli-probe",
    CliIncludeTitles => "cli-include-titles",
    CliLiveThumbnails => "cli-live-thumbnails",
    CliUsageHeading => "cli-usage-heading",
    CliCommandsHeading => "cli-commands-heading",
    CliOptionsHeading => "cli-options-heading",
    CliHelpOption => "cli-help-option",
    CliVersionOption => "cli-version-option",
    CliInvalidArguments => "cli-invalid-arguments",
    Service => "service",
    Running => "running",
    Stopped => "stopped",
    Session => "session",
    Compatible => "compatible",
    Unsupported => "unsupported",
    Capabilities => "capabilities",
    ShortcutOwnership => "shortcut-ownership",
    Owned => "owned",
    PartiallyOwned => "partially-owned",
    NotOwned => "not-owned",
    CaptureBackend => "capture-backend",
    NotNegotiated => "not-negotiated",
    DmaBuf => "dma-buf",
    SharedMemory => "shared-memory",
    CaptureBackendFallback => "capture-backend-fallback",
    IncompatibleDmaBufDevice => "incompatible-dma-buf-device",
    UnsupportedDmaBufFormat => "unsupported-dma-buf-format",
    UnsupportedDmaBufModifier => "unsupported-dma-buf-modifier",
    DmaBufAllocationFailed => "dma-buf-allocation-failed",
    DmaBufSynchronizationUnavailable => "dma-buf-synchronization-unavailable",
    DmaBufImportUnavailable => "dma-buf-import-unavailable",
    DmaBufReleaseUnavailable => "dma-buf-release-unavailable",
    MruHistory => "mru-history",
    WarmUp => "warm-up",
    Accurate => "accurate",
    WindowCount => "window-count",
    WindowScope => "window-scope",
    AllWorkspaces => "all-workspaces",
    VisibleWorkspaces => "visible-workspaces",
    WorkspaceFiltering => "workspace-filtering",
    NotRequired => "not-required",
    Required => "required",
    WorkspaceEligibility => "workspace-eligibility",
    AwaitingSnapshot => "awaiting-snapshot",
    Ready => "ready",
    Unavailable => "unavailable",
    WorkspaceEligibilityFailure => "workspace-eligibility-failure",
    NotAdvertised => "not-advertised",
    MruOrder => "mru-order",
    ToplevelInfoFailure => "toplevel-info-failure",
    WorkspaceProtocolFailure => "workspace-protocol-failure",
    WorkspaceSnapshotFailure => "workspace-snapshot-failure",
    ToplevelMembershipFailure => "toplevel-membership-failure",
);
