// SPDX-License-Identifier: GPL-3.0-only

use std::{
    error::Error,
    fmt,
    ops::{BitOr, Range},
};

mod accessibility;
mod capture;
mod localization;
mod preferences;

pub use accessibility::{
    AccessibilityPolicy, AccessibleSwitcherItem, OverlayPresentation, REVEAL_ANIMATION_DURATION,
};
pub use capture::{
    BufferTransform, CaptureBackend, CaptureBackendSelection, CaptureBackendState, CaptureEffect,
    CaptureFailure, CaptureOpportunity, CaptureSessionModel, DmaBufCompatibility,
    DmaBufContractStatus, DmaBufFallbackReason, FrameDamage, InvalidThumbnailFrame, RefreshCeiling,
    ShmConstraints, ShmFormat, ShmFrameLayout, ThumbnailFrame,
};
pub use localization::{Locale, StringKey};
pub use preferences::{
    Dimming, PreferencesStore, RevealDelay, SessionPreferences, SwitcherPreferences,
};

pub const APPLICATION_ID: &str = "io.github.abrahamv09.CosmicWindowSwitcher";

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
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
pub enum InvocationDirection {
    Next,
    Previous,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GridNavigationDirection {
    Left,
    Right,
    Up,
    Down,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvocationRequest {
    pub direction: InvocationDirection,
    pub initial_hold_modifiers: HoldModifiers,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ServiceEvent {
    SessionReady,
    SessionReadinessFailed,
    RevealDelayElapsed,
    HoldModifiersChanged(HoldModifiers),
    Switching(SwitchingEvent),
    Invocation(InvocationDirection),
    PointerEntered(Option<WindowId>),
    PointerMoved {
        window: Option<WindowId>,
        hover_selection_enabled: bool,
    },
    PointerPressed(Option<WindowId>),
    PointerReleased(Option<WindowId>),
    SessionInterrupted(SessionInterruption),
    SessionReactivated,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionInterruption {
    ScreenLock,
    Suspend,
    UserSwitch,
    OutputLoss,
    CompositorLoss,
    SessionShutdown,
}

impl SessionInterruption {
    #[must_use]
    const fn deactivates_cosmic_session(self) -> bool {
        !matches!(self, Self::OutputLoss)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionLifecycleSignal {
    Locked(bool),
    Active(bool),
    PreparingForSleep(bool),
    PreparingForShutdown(bool),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SessionBoundary {
    Inactive,
    Locked,
    PreparingForSleep,
    PreparingForShutdown,
}

#[derive(Clone, Debug)]
pub struct SessionLifecycleModel {
    boundaries: Vec<SessionBoundary>,
    interruption: Option<SessionInterruption>,
}

impl SessionLifecycleModel {
    #[must_use]
    pub fn new() -> Self {
        Self {
            boundaries: Vec::new(),
            interruption: None,
        }
    }

    pub fn handle(&mut self, signal: SessionLifecycleSignal) -> Option<ServiceEvent> {
        match signal {
            SessionLifecycleSignal::Locked(locked) => {
                self.set_boundary(SessionBoundary::Locked, locked);
            }
            SessionLifecycleSignal::Active(active) => {
                self.set_boundary(SessionBoundary::Inactive, !active);
            }
            SessionLifecycleSignal::PreparingForSleep(preparing) => {
                self.set_boundary(SessionBoundary::PreparingForSleep, preparing);
            }
            SessionLifecycleSignal::PreparingForShutdown(preparing) => {
                self.set_boundary(SessionBoundary::PreparingForShutdown, preparing);
            }
        }

        let next = if self.has_boundary(SessionBoundary::PreparingForShutdown) {
            Some(SessionInterruption::SessionShutdown)
        } else if self.has_boundary(SessionBoundary::PreparingForSleep) {
            Some(SessionInterruption::Suspend)
        } else if self.has_boundary(SessionBoundary::Locked) {
            Some(SessionInterruption::ScreenLock)
        } else if self.has_boundary(SessionBoundary::Inactive) {
            Some(SessionInterruption::UserSwitch)
        } else {
            None
        };
        let previous = std::mem::replace(&mut self.interruption, next);
        match (previous, next) {
            (None, Some(interruption)) => Some(ServiceEvent::SessionInterrupted(interruption)),
            (Some(_), None) => Some(ServiceEvent::SessionReactivated),
            (None, None) | (Some(_), Some(_)) => None,
        }
    }

    fn set_boundary(&mut self, boundary: SessionBoundary, asserted: bool) {
        if asserted {
            if !self.boundaries.contains(&boundary) {
                self.boundaries.push(boundary);
            }
        } else {
            self.boundaries.retain(|candidate| *candidate != boundary);
        }
    }

    fn has_boundary(&self, boundary: SessionBoundary) -> bool {
        self.boundaries.contains(&boundary)
    }
}

impl Default for SessionLifecycleModel {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ServiceEffect {
    PrepareInvisibleOverlay { selected: WindowId },
    RevealOverlay { selected: WindowId },
    SelectionChanged(WindowId),
    Activate(WindowId),
    Cancel,
    FallbackToStockSwitcher(InvocationDirection),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SwitchingEvent {
    Tab,
    Navigate(InvocationDirection),
    NavigateGrid {
        direction: GridNavigationDirection,
        columns: usize,
    },
    Enter,
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
        direction: InvocationDirection,
        initial_hold_modifiers: HoldModifiers,
    ) -> Result<Self, NotEnoughWindows> {
        let windows = windows.into_iter().collect::<Vec<_>>();
        if windows.len() < 2 {
            return Err(NotEnoughWindows);
        }
        let selected = match direction {
            InvocationDirection::Next => 1,
            InvocationDirection::Previous => windows.len() - 1,
        };

        Ok(Self {
            windows,
            selected,
            initial_hold_modifiers,
            state: SessionState::Open,
        })
    }

    #[must_use]
    pub fn selected(&self) -> &WindowId {
        &self.windows[self.selected]
    }

    #[must_use]
    pub fn windows(&self) -> &[WindowId] {
        &self.windows
    }

    pub fn handle(&mut self, event: SwitchingEvent) -> SessionEffect {
        if self.state == SessionState::Finished {
            return SessionEffect::None;
        }

        match event {
            SwitchingEvent::Tab => self.move_selection(InvocationDirection::Next),
            SwitchingEvent::Navigate(direction) => self.move_selection(direction),
            SwitchingEvent::NavigateGrid { direction, columns } => {
                self.move_grid_selection(direction, columns)
            }
            SwitchingEvent::Enter => {
                self.state = SessionState::Finished;
                SessionEffect::Activate(self.selected().clone())
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

    fn move_selection(&mut self, direction: InvocationDirection) -> SessionEffect {
        if self.state == SessionState::Finished {
            return SessionEffect::None;
        }
        self.selected = match direction {
            InvocationDirection::Next => (self.selected + 1) % self.windows.len(),
            InvocationDirection::Previous if self.selected == 0 => self.windows.len() - 1,
            InvocationDirection::Previous => self.selected - 1,
        };
        SessionEffect::SelectionChanged(self.selected().clone())
    }

    fn move_grid_selection(
        &mut self,
        direction: GridNavigationDirection,
        columns: usize,
    ) -> SessionEffect {
        if self.state == SessionState::Finished {
            return SessionEffect::None;
        }
        match direction {
            GridNavigationDirection::Left => {
                return self.move_selection(InvocationDirection::Previous);
            }
            GridNavigationDirection::Right => {
                return self.move_selection(InvocationDirection::Next);
            }
            GridNavigationDirection::Up | GridNavigationDirection::Down => {}
        }
        let columns = columns.max(1);
        let candidate = match direction {
            GridNavigationDirection::Up => self.selected.checked_sub(columns),
            GridNavigationDirection::Down => self
                .selected
                .checked_add(columns)
                .filter(|candidate| *candidate < self.windows.len()),
            GridNavigationDirection::Left | GridNavigationDirection::Right => {
                unreachable!("horizontal Grid Navigation returned through MRU traversal")
            }
        };
        let Some(candidate) = candidate else {
            return SessionEffect::None;
        };
        self.selected = candidate;
        SessionEffect::SelectionChanged(self.selected().clone())
    }

    fn select_window(&mut self, window: &WindowId) -> SessionEffect {
        if self.state == SessionState::Finished {
            return SessionEffect::None;
        }
        let Some(selected) = self
            .windows
            .iter()
            .position(|candidate| candidate == window)
        else {
            return SessionEffect::None;
        };
        if selected == self.selected {
            return SessionEffect::None;
        }
        self.selected = selected;
        SessionEffect::SelectionChanged(window.clone())
    }

    fn contains(&self, window: &WindowId) -> bool {
        self.windows.contains(window)
    }

    fn activate_window(&mut self, window: &WindowId) -> SessionEffect {
        if self.state == SessionState::Finished || !self.contains(window) {
            return SessionEffect::None;
        }
        self.selected = self
            .windows
            .iter()
            .position(|candidate| candidate == window)
            .expect("the checked Window belongs to the Switching Session");
        self.state = SessionState::Finished;
        SessionEffect::Activate(window.clone())
    }

    fn cancel(&mut self) -> SessionEffect {
        self.state = SessionState::Finished;
        SessionEffect::Cancelled
    }

    fn window_closed(&mut self, window: &WindowId) -> SessionEffect {
        if self.state == SessionState::Finished {
            return SessionEffect::None;
        }
        let Some(position) = self
            .windows
            .iter()
            .position(|candidate| candidate == window)
        else {
            return SessionEffect::None;
        };
        self.windows.remove(position);
        if self.windows.is_empty() {
            self.state = SessionState::Finished;
            return SessionEffect::Cancelled;
        }
        if position < self.selected {
            self.selected -= 1;
            return SessionEffect::None;
        }
        if position == self.selected {
            if self.selected == self.windows.len() {
                self.selected = 0;
            }
            return SessionEffect::SelectionChanged(self.selected().clone());
        }
        SessionEffect::None
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SessionDisplay(String);

impl From<&str> for SessionDisplay {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl From<String> for SessionDisplay {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl SessionDisplay {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SessionDisplay {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationIcon {
    name: String,
    fallback_monogram: char,
}

impl ApplicationIcon {
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub const fn fallback_monogram(&self) -> char {
        self.fallback_monogram
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SwitcherItem {
    window: WindowId,
    application_id: String,
    title: String,
    application_icon: ApplicationIcon,
    thumbnail: Option<ThumbnailFrame>,
    thumbnail_failure: Option<CaptureFailure>,
    selected: bool,
    position: usize,
    set_size: usize,
}

impl SwitcherItem {
    #[must_use]
    pub fn new(window: WindowId, application_id: String, title: String) -> Self {
        Self::new_localized(window, application_id, title, Locale::English)
    }

    #[must_use]
    pub fn new_localized(
        window: WindowId,
        application_id: String,
        title: String,
        locale: Locale,
    ) -> Self {
        let title = if title.trim().is_empty() {
            if application_id.trim().is_empty() {
                locale.text(StringKey::UntitledWindow)
            } else {
                application_id.clone()
            }
        } else {
            title
        };
        let monogram = application_id
            .rsplit(['.', '-'])
            .find_map(|part| part.chars().find(char::is_ascii_alphanumeric))
            .unwrap_or('?')
            .to_ascii_uppercase();
        let application_icon = ApplicationIcon {
            name: application_id.clone(),
            fallback_monogram: monogram,
        };

        Self {
            window,
            application_id,
            title,
            application_icon,
            thumbnail: None,
            thumbnail_failure: None,
            selected: false,
            position: 0,
            set_size: 0,
        }
    }

    #[must_use]
    pub fn window(&self) -> &WindowId {
        &self.window
    }

    #[must_use]
    pub fn application_id(&self) -> &str {
        &self.application_id
    }

    #[must_use]
    pub fn title(&self) -> &str {
        &self.title
    }

    #[must_use]
    pub const fn application_icon(&self) -> &ApplicationIcon {
        &self.application_icon
    }

    #[must_use]
    pub const fn thumbnail(&self) -> Option<&ThumbnailFrame> {
        self.thumbnail.as_ref()
    }

    pub fn update_thumbnail(&mut self, thumbnail: ThumbnailFrame) {
        self.thumbnail = Some(thumbnail);
        self.thumbnail_failure = None;
    }

    pub fn degrade_thumbnail(&mut self, reason: CaptureFailure) {
        self.thumbnail = None;
        self.thumbnail_failure = Some(reason);
    }

    #[must_use]
    pub const fn thumbnail_failure(&self) -> Option<CaptureFailure> {
        self.thumbnail_failure
    }

    #[must_use]
    pub fn accessible_name(&self) -> &str {
        &self.title
    }

    #[must_use]
    pub const fn accessible_position(&self) -> (usize, usize) {
        (self.position, self.set_size)
    }

    #[must_use]
    pub const fn is_selected(&self) -> bool {
        self.selected
    }

    #[must_use]
    pub fn accessibility(&self, locale: Locale) -> AccessibleSwitcherItem<'_> {
        AccessibleSwitcherItem::new(
            &self.title,
            self.selected,
            self.position,
            self.set_size,
            locale,
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnknownGridSelection(WindowId);

impl fmt::Display for UnknownGridSelection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "Window {} does not belong to the Switcher Grid",
            self.0
        )
    }
}

impl Error for UnknownGridSelection {}

const GRID_ITEM_GAP: u32 = 12;
const GRID_PADDING: u32 = 16;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub enum CardSize {
    Small,
    #[default]
    Medium,
    Large,
}

impl CardSize {
    #[must_use]
    pub const fn logical_size(self) -> (u32, u32) {
        match self {
            Self::Small => (240, 180),
            Self::Medium => (320, 240),
            Self::Large => (400, 300),
        }
    }

    #[must_use]
    pub fn responsive_logical_size(self, item_count: usize, display_width: u32) -> (u32, u32) {
        let width_percentage = match item_count {
            0..=2 => 40_u64,
            3 | 6 => 30,
            4 => 22,
            _ => 18,
        };
        let (density_numerator, density_denominator) = match self {
            Self::Small => (4_u64, 5_u64),
            Self::Medium => (9, 10),
            Self::Large => (1, 1),
        };
        let width = u64::from(display_width)
            .saturating_mul(width_percentage)
            .saturating_mul(density_numerator)
            / (100 * density_denominator);
        let width = u32::try_from(width)
            .unwrap_or(u32::MAX)
            .max(1)
            .min(display_width.max(1));
        (width, width.saturating_mul(3) / 4)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FractionalScale(u32);

impl FractionalScale {
    pub const UNITS_PER_ONE: u32 = 120;

    #[must_use]
    pub const fn from_protocol_units(units: u32) -> Self {
        Self(if units == 0 { 1 } else { units })
    }

    #[must_use]
    pub const fn from_integer(scale: u32) -> Self {
        Self(if scale == 0 { 1 } else { scale }.saturating_mul(Self::UNITS_PER_ONE))
    }

    #[must_use]
    pub const fn protocol_units(self) -> u32 {
        self.0
    }

    #[must_use]
    pub fn physical_length(self, logical: u32) -> u32 {
        let scaled = u64::from(logical)
            .saturating_mul(u64::from(self.0))
            .div_ceil(u64::from(Self::UNITS_PER_ONE));
        u32::try_from(scaled).unwrap_or(u32::MAX)
    }

    #[must_use]
    pub fn physical_size(self, logical_width: u32, logical_height: u32) -> (u32, u32) {
        (
            self.physical_length(logical_width),
            self.physical_length(logical_height),
        )
    }

    #[must_use]
    pub fn ceiling_integer(self) -> i32 {
        i32::try_from(self.0.div_ceil(Self::UNITS_PER_ONE)).unwrap_or(i32::MAX)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GridRect {
    x: i64,
    y: i64,
    width: u32,
    height: u32,
}

impl GridRect {
    #[must_use]
    pub fn x(self) -> u32 {
        u32::try_from(self.x).unwrap_or_else(|_| if self.x.is_negative() { 0 } else { u32::MAX })
    }

    #[must_use]
    pub fn y(self) -> u32 {
        u32::try_from(self.y).unwrap_or_else(|_| if self.y.is_negative() { 0 } else { u32::MAX })
    }

    #[must_use]
    pub const fn signed_x(self) -> i64 {
        self.x
    }

    #[must_use]
    pub const fn signed_y(self) -> i64 {
        self.y
    }

    #[must_use]
    pub const fn size(self) -> (u32, u32) {
        (self.width, self.height)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GridLayout {
    columns: usize,
    total_rows: usize,
    visible_rows: usize,
    visible_item_range: Range<usize>,
    logical_size: (u32, u32),
    card_size: (u32, u32),
    origin: (u32, u32),
    leading_peek: bool,
}

impl GridLayout {
    #[must_use]
    pub const fn columns(&self) -> usize {
        self.columns
    }

    #[must_use]
    pub const fn total_rows(&self) -> usize {
        self.total_rows
    }

    #[must_use]
    pub const fn visible_rows(&self) -> usize {
        self.visible_rows
    }

    #[must_use]
    pub fn visible_item_range(&self) -> Range<usize> {
        self.visible_item_range.clone()
    }

    #[must_use]
    pub const fn logical_size(&self) -> (u32, u32) {
        self.logical_size
    }

    #[must_use]
    pub const fn viewport_bounds(&self) -> GridRect {
        GridRect {
            x: self.origin.0 as i64,
            y: self.origin.1 as i64,
            width: self.logical_size.0,
            height: self.logical_size.1,
        }
    }

    #[must_use]
    pub fn centered_in(mut self, logical_width: u32, logical_height: u32) -> Self {
        self.origin = (
            logical_width.saturating_sub(self.logical_size.0) / 2,
            logical_height.saturating_sub(self.logical_size.1) / 2,
        );
        self
    }

    #[must_use]
    pub fn item_bounds(&self, item_index: usize) -> Option<GridRect> {
        if !self.visible_item_range.contains(&item_index) {
            return None;
        }
        let visible_index = item_index - self.visible_item_range.start;
        let column = visible_index % self.columns;
        let row = visible_index / self.columns;
        let (width, height) = self.card_size;
        let leading_offset = if self.leading_peek { height / 2 } else { 0 };
        Some(GridRect {
            x: i64::from(self.origin.0)
                + i64::from(GRID_PADDING)
                + i64::from(u32::try_from(column).ok()?)
                    * i64::from(width.saturating_add(GRID_ITEM_GAP)),
            y: i64::from(self.origin.1) + i64::from(GRID_PADDING) - i64::from(leading_offset)
                + i64::from(u32::try_from(row).ok()?)
                    * i64::from(height.saturating_add(GRID_ITEM_GAP)),
            width,
            height,
        })
    }

    #[must_use]
    pub fn item_at(&self, x: f64, y: f64) -> Option<usize> {
        let viewport = self.viewport_bounds();
        if x < logical_coordinate_as_f64(viewport.x)
            || x >= logical_coordinate_as_f64(viewport.x.saturating_add(i64::from(viewport.width)))
            || y < logical_coordinate_as_f64(viewport.y)
            || y >= logical_coordinate_as_f64(viewport.y.saturating_add(i64::from(viewport.height)))
        {
            return None;
        }
        self.visible_item_range.clone().find(|item_index| {
            self.item_bounds(*item_index).is_some_and(|bounds| {
                let right = bounds.x.saturating_add(i64::from(bounds.width));
                let bottom = bounds.y.saturating_add(i64::from(bounds.height));
                x >= logical_coordinate_as_f64(bounds.x)
                    && x < logical_coordinate_as_f64(right)
                    && y >= logical_coordinate_as_f64(bounds.y)
                    && y < logical_coordinate_as_f64(bottom)
            })
        })
    }
}

fn logical_coordinate_as_f64(coordinate: i64) -> f64 {
    let coordinate = i32::try_from(coordinate).unwrap_or_else(|_| {
        if coordinate.is_negative() {
            i32::MIN
        } else {
            i32::MAX
        }
    });
    f64::from(coordinate)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SwitcherGrid {
    session_display: SessionDisplay,
    items: Vec<SwitcherItem>,
    first_visible_row: usize,
}

impl SwitcherGrid {
    /// Builds the stable item order for one Switching Session.
    ///
    /// # Errors
    ///
    /// Returns [`UnknownGridSelection`] when `selected` is not represented by
    /// one of the supplied items.
    pub fn new(
        session_display: SessionDisplay,
        items: impl IntoIterator<Item = SwitcherItem>,
        selected: &WindowId,
    ) -> Result<Self, UnknownGridSelection> {
        let mut items = items.into_iter().collect::<Vec<_>>();
        update_accessible_positions(&mut items);
        let mut grid = Self {
            session_display,
            items,
            first_visible_row: 0,
        };
        grid.select(selected)?;
        Ok(grid)
    }

    #[must_use]
    pub const fn session_display(&self) -> &SessionDisplay {
        &self.session_display
    }

    #[must_use]
    pub fn items(&self) -> &[SwitcherItem] {
        &self.items
    }

    #[must_use]
    pub fn selected_window(&self) -> Option<&WindowId> {
        self.items
            .iter()
            .find(|item| item.is_selected())
            .map(SwitcherItem::window)
    }

    #[must_use]
    pub fn window_at(&self, layout: &GridLayout, x: f64, y: f64) -> Option<&WindowId> {
        let item_index = layout.item_at(x, y)?;
        self.items.get(item_index).map(SwitcherItem::window)
    }

    #[must_use]
    pub fn visible_windows(&self, layout: &GridLayout) -> Vec<WindowId> {
        self.items
            .get(layout.visible_item_range())
            .unwrap_or_default()
            .iter()
            .map(|item| item.window().clone())
            .collect()
    }

    /// Returns the item range for a viewport that scrolls only enough to keep
    /// the selected row visible.
    ///
    /// `columns` and `visible_rows` are normalized to at least one.
    pub fn visible_item_range(&mut self, columns: usize, visible_rows: usize) -> Range<usize> {
        if self.items.is_empty() {
            self.first_visible_row = 0;
            return 0..0;
        }
        let columns = columns.max(1);
        let rows = self.items.len().div_ceil(columns);
        let visible_rows = visible_rows.max(1).min(rows);
        let selected_index = self
            .items
            .iter()
            .position(SwitcherItem::is_selected)
            .unwrap_or(0);
        let selected_row = selected_index / columns;
        self.first_visible_row = self
            .first_visible_row
            .min(rows.saturating_sub(visible_rows));
        if selected_row < self.first_visible_row {
            self.first_visible_row = selected_row;
        } else if selected_row >= self.first_visible_row + visible_rows {
            self.first_visible_row = selected_row + 1 - visible_rows;
        }

        let first_item = self.first_visible_row * columns;
        let last_item = (first_item + visible_rows * columns).min(self.items.len());
        first_item..last_item
    }

    #[must_use]
    pub fn layout(
        &mut self,
        maximum_logical_width: u32,
        maximum_logical_height: u32,
        card_size: CardSize,
    ) -> GridLayout {
        self.layout_with_card_dimensions(
            maximum_logical_width,
            maximum_logical_height,
            card_size.logical_size(),
            5,
        )
    }

    #[must_use]
    pub fn responsive_layout(
        &mut self,
        display_logical_width: u32,
        display_logical_height: u32,
        card_size: CardSize,
    ) -> GridLayout {
        let maximum_width = display_logical_width.saturating_mul(19) / 20;
        let maximum_height = display_logical_height.saturating_mul(19) / 20;
        let columns = responsive_columns(self.items.len());
        let total_rows = self.items.len().div_ceil(columns);
        let columns_u32 = u32::try_from(columns).unwrap_or(u32::MAX);
        let horizontal_overhead =
            2 * GRID_PADDING + columns_u32.saturating_sub(1).saturating_mul(GRID_ITEM_GAP);
        let maximum_card_width = maximum_width
            .saturating_sub(horizontal_overhead)
            .checked_div(columns_u32.max(1))
            .unwrap_or(0)
            .max(1);
        let target_card =
            card_size.responsive_logical_size(self.items.len(), display_logical_width);
        let fitted_width = target_card.0.min(maximum_card_width);
        let card_dimensions = fit_responsive_card_height(
            (fitted_width, fitted_width.saturating_mul(3) / 4),
            maximum_height,
            total_rows,
        );
        self.layout_with_card_dimensions(maximum_width, maximum_height, card_dimensions, columns)
    }

    fn layout_with_card_dimensions(
        &mut self,
        maximum_logical_width: u32,
        maximum_logical_height: u32,
        card_size: (u32, u32),
        maximum_columns: usize,
    ) -> GridLayout {
        let (card_width, card_height) = card_size;
        let available_width =
            maximum_logical_width.max(card_width.saturating_add(2 * GRID_PADDING));
        let columns = ((available_width - 2 * GRID_PADDING + GRID_ITEM_GAP)
            / (card_width + GRID_ITEM_GAP))
            .max(1);
        let item_count = self.items.len();
        let columns = usize::try_from(columns)
            .unwrap_or(usize::MAX)
            .min(maximum_columns.max(1))
            .min(item_count.max(1));
        let total_rows = item_count.div_ceil(columns);
        let available_height =
            maximum_logical_height.max(card_height.saturating_add(2 * GRID_PADDING));
        let maximum_visible_rows = ((available_height - 2 * GRID_PADDING + GRID_ITEM_GAP)
            / (card_height + GRID_ITEM_GAP))
            .max(1);
        let visible_rows = total_rows
            .min(usize::try_from(maximum_visible_rows).unwrap_or(usize::MAX))
            .max(usize::from(!self.items.is_empty()));
        let fully_visible_item_range = self.visible_item_range(columns, visible_rows);
        let columns_u32 = u32::try_from(columns).unwrap_or(u32::MAX);
        let visible_rows_u32 = u32::try_from(visible_rows).unwrap_or(u32::MAX);
        let logical_width = 2 * GRID_PADDING
            + columns_u32.saturating_mul(card_width)
            + columns_u32.saturating_sub(1).saturating_mul(GRID_ITEM_GAP);
        let fully_visible_height = 2 * GRID_PADDING
            + visible_rows_u32.saturating_mul(card_height)
            + visible_rows_u32
                .saturating_sub(1)
                .saturating_mul(GRID_ITEM_GAP);
        let peek_height = card_height / 2;
        let has_leading_row = fully_visible_item_range.start > 0;
        let has_trailing_row = fully_visible_item_range.end < item_count;
        let peek_fits = (has_leading_row || has_trailing_row)
            && fully_visible_height
                .saturating_add(GRID_ITEM_GAP)
                .saturating_add(peek_height)
                <= available_height;
        let leading_peek = peek_fits && has_leading_row;
        let visible_item_range = if leading_peek {
            fully_visible_item_range.start.saturating_sub(columns)..fully_visible_item_range.end
        } else if peek_fits {
            fully_visible_item_range.start
                ..fully_visible_item_range
                    .end
                    .saturating_add(columns)
                    .min(item_count)
        } else {
            fully_visible_item_range
        };
        let logical_height = if peek_fits {
            fully_visible_height
                .saturating_add(GRID_ITEM_GAP)
                .saturating_add(peek_height)
        } else {
            fully_visible_height
        };

        GridLayout {
            columns,
            total_rows,
            visible_rows,
            visible_item_range,
            logical_size: (logical_width, logical_height),
            card_size: (card_width, card_height),
            origin: (0, 0),
            leading_peek,
        }
    }

    /// Changes the selected item without changing MRU Order.
    ///
    /// # Errors
    ///
    /// Returns [`UnknownGridSelection`] when `selected` is not represented in
    /// this Switching Session.
    pub fn select(&mut self, selected: &WindowId) -> Result<(), UnknownGridSelection> {
        if !self.items.iter().any(|item| item.window == *selected) {
            return Err(UnknownGridSelection(selected.clone()));
        }
        for item in &mut self.items {
            item.selected = item.window == *selected;
        }
        Ok(())
    }

    pub fn remove(&mut self, window: &WindowId) -> bool {
        let Some(index) = self.items.iter().position(|item| item.window == *window) else {
            return false;
        };
        self.items.remove(index);
        update_accessible_positions(&mut self.items);
        true
    }

    pub fn update_thumbnail(&mut self, window: &WindowId, thumbnail: ThumbnailFrame) -> bool {
        let Some(item) = self.items.iter_mut().find(|item| item.window == *window) else {
            return false;
        };
        item.update_thumbnail(thumbnail);
        true
    }

    pub fn degrade_thumbnail(&mut self, window: &WindowId, reason: CaptureFailure) -> bool {
        let Some(item) = self.items.iter_mut().find(|item| item.window == *window) else {
            return false;
        };
        item.degrade_thumbnail(reason);
        true
    }
}

fn responsive_columns(item_count: usize) -> usize {
    match item_count {
        0 => 1,
        1..=5 => item_count,
        6 => 3,
        _ => 5,
    }
}

fn fit_responsive_card_height(
    card_size: (u32, u32),
    maximum_height: u32,
    total_rows: usize,
) -> (u32, u32) {
    let fully_visible_rows = total_rows.min(2);
    let includes_peek = total_rows > 2;
    let gap_count = fully_visible_rows
        .saturating_sub(1)
        .saturating_add(usize::from(includes_peek));
    let overhead = 2 * GRID_PADDING
        + u32::try_from(gap_count)
            .unwrap_or(u32::MAX)
            .saturating_mul(GRID_ITEM_GAP);
    let available_for_cards = maximum_height.saturating_sub(overhead).max(1);
    let maximum_card_height = if includes_peek {
        available_for_cards.saturating_mul(2) / 5
    } else {
        available_for_cards / u32::try_from(fully_visible_rows.max(1)).unwrap_or(u32::MAX)
    }
    .max(1);
    if card_size.1 <= maximum_card_height {
        return card_size;
    }
    let width = maximum_card_height.saturating_mul(4) / 3;
    (width.max(1), width.saturating_mul(3) / 4)
}

fn update_accessible_positions(items: &mut [SwitcherItem]) {
    let set_size = items.len();
    for (index, item) in items.iter_mut().enumerate() {
        item.position = index + 1;
        item.set_size = set_size;
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WorkspaceId(String);

impl From<&str> for WorkspaceId {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl From<String> for WorkspaceId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl WorkspaceId {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for WorkspaceId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkspaceGroupSnapshot {
    pub outputs: Vec<SessionDisplay>,
    pub workspaces: Vec<WorkspaceId>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkspaceSnapshot {
    pub id: WorkspaceId,
    pub active: bool,
    pub hidden: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WindowSnapshot {
    pub id: WindowId,
    pub workspace_membership: Vec<WorkspaceId>,
    pub output_membership: Vec<SessionDisplay>,
    pub session_display: Option<SessionDisplay>,
    pub minimized: bool,
    pub fullscreen: bool,
    pub sticky: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WindowScope {
    #[default]
    AllWorkspaces,
    VisibleWorkspaces,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DesktopSnapshot {
    pub workspace_groups: Vec<WorkspaceGroupSnapshot>,
    pub workspaces: Vec<WorkspaceSnapshot>,
    pub windows: Vec<WindowSnapshot>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SwitchingContext {
    pub eligible_windows: Vec<WindowId>,
    pub session_display: SessionDisplay,
}

impl DesktopSnapshot {
    /// Derives the immutable Window and display snapshot for one invocation.
    ///
    /// All Workspaces falls back to a deterministic workspace-group output
    /// when the focused Window has no output membership. Visible Workspaces
    /// returns `None` in that case because it requires an authoritative Session
    /// Display.
    #[must_use]
    pub fn switching_context(
        &self,
        scope: WindowScope,
        mru_order: impl IntoIterator<Item = WindowId>,
    ) -> Option<SwitchingContext> {
        let mru_order = mru_order.into_iter().collect::<Vec<_>>();
        let focused = mru_order.first()?;
        let session_display = self
            .windows
            .iter()
            .find(|window| window.id == *focused)?
            .session_display
            .clone()
            .or_else(|| {
                (scope == WindowScope::AllWorkspaces).then(|| {
                    self.workspace_groups
                        .iter()
                        .flat_map(|group| &group.outputs)
                        .min_by_key(|display| display.as_str())
                        .cloned()
                })?
            })?;
        let visible_workspaces = self
            .workspace_groups
            .iter()
            .filter(|group| !group.outputs.is_empty())
            .flat_map(|group| &group.workspaces)
            .filter(|workspace_id| {
                self.workspaces.iter().any(|workspace| {
                    workspace.id == **workspace_id && workspace.active && !workspace.hidden
                })
            })
            .collect::<std::collections::HashSet<_>>();
        let eligible_windows = mru_order
            .into_iter()
            .filter(|id| {
                self.windows.iter().any(|window| {
                    window.id == *id
                        && (scope == WindowScope::AllWorkspaces
                            || window.sticky
                            || window
                                .workspace_membership
                                .iter()
                                .any(|workspace| visible_workspaces.contains(workspace)))
                })
            })
            .collect();

        Some(SwitchingContext {
            eligible_windows,
            session_display,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MruHistoryAccuracy {
    WarmUp,
    Accurate,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WorkspaceEligibilityState {
    #[default]
    AwaitingSnapshot,
    Ready,
    MissingToplevelInfo {
        advertised_version: Option<u32>,
        required_version: u32,
    },
    MissingWorkspaceProtocol {
        advertised_version: Option<u32>,
        required_version: u32,
    },
    MissingWorkspaceSnapshot {
        advertised_version: u32,
    },
    MissingToplevelMembership {
        advertised_version: u32,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServiceDiagnostics {
    pub mru_history: MruHistoryAccuracy,
    pub mru_order: Vec<WindowId>,
    pub window_scope: WindowScope,
    pub workspace_eligibility: WorkspaceEligibilityState,
    pub capture_backend: CaptureBackendState,
}

impl ServiceDiagnostics {
    #[must_use]
    pub fn localized(&self, locale: Locale) -> String {
        let mru_history = match self.mru_history {
            MruHistoryAccuracy::WarmUp => StringKey::WarmUp,
            MruHistoryAccuracy::Accurate => StringKey::Accurate,
        };
        let window_scope = match self.window_scope {
            WindowScope::AllWorkspaces => StringKey::AllWorkspaces,
            WindowScope::VisibleWorkspaces => StringKey::VisibleWorkspaces,
        };
        let workspace_filtering = match self.window_scope {
            WindowScope::AllWorkspaces => StringKey::NotRequired,
            WindowScope::VisibleWorkspaces => StringKey::Required,
        };
        let mut lines = vec![
            localized_status(locale, StringKey::Service, StringKey::Running),
            localized_status(
                locale,
                StringKey::CaptureBackend,
                capture_backend_key(self.capture_backend),
            ),
            localized_status(locale, StringKey::MruHistory, mru_history),
            format!(
                "{}: {}",
                locale.text(StringKey::WindowCount),
                self.mru_order.len()
            ),
            localized_status(locale, StringKey::WindowScope, window_scope),
            localized_status(locale, StringKey::WorkspaceFiltering, workspace_filtering),
        ];
        if let Some(reason) = self
            .capture_backend
            .selection()
            .and_then(CaptureBackendSelection::fallback_reason)
        {
            lines.insert(
                2,
                localized_status(
                    locale,
                    StringKey::CaptureBackendFallback,
                    match reason {
                        DmaBufFallbackReason::IncompatibleDevice => {
                            StringKey::IncompatibleDmaBufDevice
                        }
                        DmaBufFallbackReason::UnsupportedFormat => {
                            StringKey::UnsupportedDmaBufFormat
                        }
                        DmaBufFallbackReason::UnsupportedModifier => {
                            StringKey::UnsupportedDmaBufModifier
                        }
                        DmaBufFallbackReason::AllocationFailed => StringKey::DmaBufAllocationFailed,
                        DmaBufFallbackReason::SynchronizationUnavailable => {
                            StringKey::DmaBufSynchronizationUnavailable
                        }
                        DmaBufFallbackReason::ImportUnavailable => {
                            StringKey::DmaBufImportUnavailable
                        }
                        DmaBufFallbackReason::ReleaseUnavailable => {
                            StringKey::DmaBufReleaseUnavailable
                        }
                    },
                ),
            );
        }
        lines.extend(localized_workspace_eligibility(
            locale,
            self.workspace_eligibility,
        ));
        if !self.mru_order.is_empty() {
            lines.push(format!("{}:", locale.text(StringKey::MruOrder)));
            lines.extend(
                self.mru_order
                    .iter()
                    .enumerate()
                    .map(|(position, id)| format!("  {}. {id}", position + 1)),
            );
        }
        lines.join("\n")
    }
}

fn capture_backend_key(state: CaptureBackendState) -> StringKey {
    match state.selection().map(CaptureBackendSelection::backend) {
        Some(CaptureBackend::DmaBuf) => StringKey::DmaBuf,
        Some(CaptureBackend::SharedMemory) => StringKey::SharedMemory,
        None => StringKey::NotNegotiated,
    }
}

fn localized_status(locale: Locale, label: StringKey, value: StringKey) -> String {
    format!("{}: {}", locale.text(label), locale.text(value))
}

fn localized_workspace_eligibility(
    locale: Locale,
    eligibility: WorkspaceEligibilityState,
) -> Vec<String> {
    let value = match eligibility {
        WorkspaceEligibilityState::AwaitingSnapshot => StringKey::AwaitingSnapshot,
        WorkspaceEligibilityState::Ready => StringKey::Ready,
        WorkspaceEligibilityState::MissingToplevelInfo { .. }
        | WorkspaceEligibilityState::MissingWorkspaceProtocol { .. }
        | WorkspaceEligibilityState::MissingWorkspaceSnapshot { .. }
        | WorkspaceEligibilityState::MissingToplevelMembership { .. } => StringKey::Unavailable,
    };
    let mut lines = vec![localized_status(
        locale,
        StringKey::WorkspaceEligibility,
        value,
    )];
    let failure = match eligibility {
        WorkspaceEligibilityState::AwaitingSnapshot | WorkspaceEligibilityState::Ready => None,
        WorkspaceEligibilityState::MissingToplevelInfo {
            advertised_version,
            required_version,
        } => Some(localized_failure(
            locale,
            StringKey::ToplevelInfoFailure,
            localized_advertised_version(locale, advertised_version),
            required_version,
        )),
        WorkspaceEligibilityState::MissingWorkspaceProtocol {
            advertised_version,
            required_version,
        } => Some(localized_failure(
            locale,
            StringKey::WorkspaceProtocolFailure,
            localized_advertised_version(locale, advertised_version),
            required_version,
        )),
        WorkspaceEligibilityState::MissingWorkspaceSnapshot { advertised_version } => {
            Some(localized_failure(
                locale,
                StringKey::WorkspaceSnapshotFailure,
                advertised_version.to_string(),
                advertised_version,
            ))
        }
        WorkspaceEligibilityState::MissingToplevelMembership { advertised_version } => {
            Some(localized_failure(
                locale,
                StringKey::ToplevelMembershipFailure,
                advertised_version.to_string(),
                advertised_version,
            ))
        }
    };
    lines.extend(failure);
    lines
}

fn localized_advertised_version(locale: Locale, version: Option<u32>) -> String {
    version.map_or_else(
        || locale.text(StringKey::NotAdvertised),
        |version| format!("v{version}"),
    )
}

fn localized_failure(
    locale: Locale,
    message: StringKey,
    advertised: String,
    required: u32,
) -> String {
    let mut arguments = fluent_bundle::FluentArgs::new();
    arguments.set("advertised", advertised);
    arguments.set("required", required);
    format!(
        "{}: {}",
        locale.text(StringKey::WorkspaceEligibilityFailure),
        locale.format(message, Some(&arguments))
    )
}

impl fmt::Display for ServiceDiagnostics {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.localized(Locale::English))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WindowEvent {
    Discovered(WindowId),
    Activated(WindowId),
    Closed(WindowId),
}

#[derive(Clone, Debug)]
struct TrackedWindow {
    id: WindowId,
    recency_known: bool,
}

#[derive(Clone, Debug)]
struct ActiveSession {
    session: SwitchingSession,
    direction: InvocationDirection,
    ready: bool,
    reveal_due: bool,
    revealed: bool,
    activation_after_readiness: Option<WindowId>,
    pointer_press: PointerPress,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
enum PointerPress {
    #[default]
    None,
    Outside,
    Item(WindowId),
}

impl ActiveSession {
    fn translate_session_effect(&mut self, effect: SessionEffect) -> (Vec<ServiceEffect>, bool) {
        match effect {
            SessionEffect::SelectionChanged(window) => {
                (vec![ServiceEffect::SelectionChanged(window)], false)
            }
            SessionEffect::Activate(window) if self.ready => {
                (vec![ServiceEffect::Activate(window)], true)
            }
            SessionEffect::Activate(window) => {
                self.activation_after_readiness = Some(window);
                (Vec::new(), false)
            }
            SessionEffect::Cancelled => (vec![ServiceEffect::Cancel], true),
            SessionEffect::None => (Vec::new(), false),
        }
    }

    fn handle_pointer_event(&mut self, event: ServiceEvent) -> SessionEffect {
        if !self.revealed {
            return SessionEffect::None;
        }
        match event {
            ServiceEvent::PointerMoved {
                window: Some(window),
                hover_selection_enabled: true,
            } if self.pointer_press == PointerPress::None =>
            {
                self.session.select_window(&window)
            }
            ServiceEvent::PointerPressed(Some(window)) if self.session.contains(&window) => {
                self.pointer_press = PointerPress::Item(window);
                SessionEffect::None
            }
            ServiceEvent::PointerPressed(None) => {
                self.pointer_press = PointerPress::Outside;
                SessionEffect::None
            }
            ServiceEvent::PointerReleased(released_over) => {
                let pressed = std::mem::take(&mut self.pointer_press);
                match (pressed, released_over) {
                    (PointerPress::Item(pressed), Some(released)) if pressed == released => {
                        self.session.activate_window(&released)
                    }
                    (PointerPress::Outside, None) => self.session.cancel(),
                    _ => SessionEffect::None,
                }
            }
            ServiceEvent::PointerEntered(_)
            | ServiceEvent::PointerMoved { .. }
            | ServiceEvent::PointerPressed(_) => SessionEffect::None,
            ServiceEvent::SessionReady
            | ServiceEvent::SessionReadinessFailed
            | ServiceEvent::RevealDelayElapsed
            | ServiceEvent::HoldModifiersChanged(_)
            | ServiceEvent::Switching(_)
            | ServiceEvent::Invocation(_)
            | ServiceEvent::SessionInterrupted(_)
            | ServiceEvent::SessionReactivated => {
                unreachable!("only pointer events are passed to the pointer handler")
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct SwitcherService {
    windows: Vec<TrackedWindow>,
    initial_discovery_complete: bool,
    active_session: Option<ActiveSession>,
    window_scope: WindowScope,
    workspace_eligibility: WorkspaceEligibilityState,
    capture_backend: CaptureBackendState,
    cosmic_session_active: bool,
}

impl Default for SwitcherService {
    fn default() -> Self {
        Self::new()
    }
}

impl SwitcherService {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            windows: Vec::new(),
            initial_discovery_complete: false,
            active_session: None,
            window_scope: WindowScope::AllWorkspaces,
            workspace_eligibility: WorkspaceEligibilityState::AwaitingSnapshot,
            capture_backend: CaptureBackendState::NotNegotiated,
            cosmic_session_active: true,
        }
    }

    pub const fn complete_initial_discovery(&mut self) {
        self.initial_discovery_complete = true;
    }

    pub const fn set_workspace_eligibility_state(&mut self, state: WorkspaceEligibilityState) {
        self.workspace_eligibility = state;
    }

    pub const fn set_window_scope(&mut self, scope: WindowScope) {
        self.window_scope = scope;
    }

    pub const fn set_capture_backend(&mut self, selection: CaptureBackendSelection) {
        self.capture_backend = CaptureBackendState::active(selection);
    }

    #[must_use]
    pub const fn window_scope(&self) -> WindowScope {
        self.window_scope
    }

    #[must_use]
    pub const fn workspace_invocation_fallback(
        &self,
        direction: InvocationDirection,
    ) -> Option<ServiceEffect> {
        if matches!(self.window_scope, WindowScope::AllWorkspaces) {
            return None;
        }
        match self.workspace_eligibility {
            WorkspaceEligibilityState::Ready => None,
            WorkspaceEligibilityState::AwaitingSnapshot
            | WorkspaceEligibilityState::MissingToplevelInfo { .. }
            | WorkspaceEligibilityState::MissingWorkspaceProtocol { .. }
            | WorkspaceEligibilityState::MissingWorkspaceSnapshot { .. }
            | WorkspaceEligibilityState::MissingToplevelMembership { .. } => {
                Some(ServiceEffect::FallbackToStockSwitcher(direction))
            }
        }
    }

    pub fn observe(&mut self, event: WindowEvent) -> Vec<ServiceEffect> {
        if !self.cosmic_session_active {
            return Vec::new();
        }
        let closed = match &event {
            WindowEvent::Closed(id) => Some(id.clone()),
            _ => None,
        };
        match event {
            WindowEvent::Discovered(id) => {
                if !self.windows.iter().any(|window| window.id == id) {
                    self.windows.push(TrackedWindow {
                        id,
                        recency_known: self.initial_discovery_complete,
                    });
                }
            }
            WindowEvent::Activated(id) => {
                if let Some(position) = self.windows.iter().position(|window| window.id == id) {
                    let mut window = self.windows.remove(position);
                    window.recency_known = true;
                    self.windows.insert(0, window);
                }
            }
            WindowEvent::Closed(id) => {
                self.windows.retain(|window| window.id != id);
            }
        }

        if let Some(closed) = closed
            && self.active_session.is_some()
        {
            return self.apply_active_session_event(|active_session| {
                active_session.session.window_closed(&closed)
            });
        }

        Vec::new()
    }

    pub fn invoke(&mut self, request: InvocationRequest) -> Vec<ServiceEffect> {
        let windows = self
            .windows
            .iter()
            .map(|window| window.id.clone())
            .collect::<Vec<_>>();
        self.invoke_for_window_set(request, windows)
    }

    /// Starts switching from a Session Window Set captured with the Invocation Request.
    ///
    /// The caller owns the atomic snapshot boundary. Later Window discovery and
    /// MRU changes cannot enter the resulting Switching Session.
    pub fn invoke_for_window_set(
        &mut self,
        request: InvocationRequest,
        windows: impl IntoIterator<Item = WindowId>,
    ) -> Vec<ServiceEffect> {
        if !self.cosmic_session_active {
            return Vec::new();
        }
        let Ok(session) =
            SwitchingSession::new(windows, request.direction, request.initial_hold_modifiers)
        else {
            return Vec::new();
        };
        let selected = session.selected().clone();
        self.active_session = Some(ActiveSession {
            session,
            direction: request.direction,
            ready: false,
            reveal_due: false,
            revealed: false,
            activation_after_readiness: None,
            pointer_press: PointerPress::None,
        });
        vec![ServiceEffect::PrepareInvisibleOverlay { selected }]
    }

    pub fn handle(&mut self, event: ServiceEvent) -> Vec<ServiceEffect> {
        match event {
            ServiceEvent::SessionInterrupted(interruption) => {
                let active_session = self.active_session.take();
                if interruption.deactivates_cosmic_session() {
                    self.cosmic_session_active = false;
                    self.windows.clear();
                    self.initial_discovery_complete = false;
                }
                return match active_session {
                    Some(active_session)
                        if interruption == SessionInterruption::OutputLoss
                            && !active_session.revealed =>
                    {
                        vec![ServiceEffect::FallbackToStockSwitcher(
                            active_session.direction,
                        )]
                    }
                    Some(_) => vec![ServiceEffect::Cancel],
                    None => Vec::new(),
                };
            }
            ServiceEvent::SessionReactivated => {
                self.cosmic_session_active = true;
                return Vec::new();
            }
            _ => {}
        }
        if self.active_session.is_none() {
            return Vec::new();
        }

        match event {
            ServiceEvent::SessionReadinessFailed => {
                let Some(active_session) = self.active_session.as_ref() else {
                    return Vec::new();
                };
                let direction = active_session.direction;
                self.active_session = None;
                vec![ServiceEffect::FallbackToStockSwitcher(direction)]
            }
            ServiceEvent::SessionReady => {
                let Some(active_session) = self.active_session.as_mut() else {
                    return Vec::new();
                };
                active_session.ready = true;
                if let Some(window) = active_session.activation_after_readiness.take() {
                    self.active_session = None;
                    vec![ServiceEffect::Activate(window)]
                } else if active_session.reveal_due && !active_session.revealed {
                    active_session.revealed = true;
                    vec![ServiceEffect::RevealOverlay {
                        selected: active_session.session.selected().clone(),
                    }]
                } else {
                    Vec::new()
                }
            }
            ServiceEvent::RevealDelayElapsed => {
                let Some(active_session) = self.active_session.as_mut() else {
                    return Vec::new();
                };
                active_session.reveal_due = true;
                if active_session.ready && !active_session.revealed {
                    active_session.revealed = true;
                    vec![ServiceEffect::RevealOverlay {
                        selected: active_session.session.selected().clone(),
                    }]
                } else {
                    Vec::new()
                }
            }
            ServiceEvent::HoldModifiersChanged(modifiers) => {
                self.apply_active_session_event(|active_session| {
                    active_session
                        .session
                        .handle(SwitchingEvent::HoldModifiersChanged(modifiers))
                })
            }
            ServiceEvent::Switching(event) => self
                .apply_active_session_event(|active_session| active_session.session.handle(event)),
            ServiceEvent::Invocation(direction) => {
                self.apply_active_session_event(|active_session| {
                    active_session.session.move_selection(direction)
                })
            }
            event @ (ServiceEvent::PointerEntered(_)
            | ServiceEvent::PointerMoved { .. }
            | ServiceEvent::PointerPressed(_)
            | ServiceEvent::PointerReleased(_)) => {
                self.apply_active_session_event(|active_session| {
                    active_session.handle_pointer_event(event)
                })
            }
            ServiceEvent::SessionInterrupted(_) | ServiceEvent::SessionReactivated => {
                unreachable!("lifecycle events returned before active-session dispatch")
            }
        }
    }

    fn apply_active_session_event(
        &mut self,
        event: impl FnOnce(&mut ActiveSession) -> SessionEffect,
    ) -> Vec<ServiceEffect> {
        let active_session = self
            .active_session
            .as_mut()
            .expect("the active Switching Session was checked");
        let effect = event(active_session);
        let (effects, finished) = active_session.translate_session_effect(effect);
        if finished {
            self.active_session = None;
        }
        effects
    }

    #[must_use]
    pub fn diagnostics(&self) -> ServiceDiagnostics {
        ServiceDiagnostics {
            mru_history: if self.windows.iter().all(|window| window.recency_known) {
                MruHistoryAccuracy::Accurate
            } else {
                MruHistoryAccuracy::WarmUp
            },
            mru_order: self
                .windows
                .iter()
                .map(|window| window.id.clone())
                .collect(),
            window_scope: self.window_scope,
            workspace_eligibility: self.workspace_eligibility,
            capture_backend: self.capture_backend,
        }
    }
}
