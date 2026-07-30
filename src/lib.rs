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
pub enum InvocationDirection {
    Next,
    Previous,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvocationRequest {
    pub direction: InvocationDirection,
    pub initial_hold_modifiers: HoldModifiers,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServiceEvent {
    SessionReady,
    SessionReadinessFailed,
    RevealDelayElapsed,
    HoldModifiersChanged(HoldModifiers),
    Switching(SwitchingEvent),
    Invocation(InvocationDirection),
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

#[derive(Clone, Debug, PartialEq, Eq)]
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
pub enum ApplicationIcon {
    Monogram(char),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SwitcherCard {
    window: WindowId,
    application_id: String,
    title: String,
    application_icon: ApplicationIcon,
    selected: bool,
    position: usize,
    set_size: usize,
}

impl SwitcherCard {
    #[must_use]
    pub fn new(window: WindowId, application_id: String, title: String) -> Self {
        let title = if title.trim().is_empty() {
            if application_id.trim().is_empty() {
                "Untitled Window".to_owned()
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

        Self {
            window,
            application_id,
            title,
            application_icon: ApplicationIcon::Monogram(monogram),
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SwitcherGrid {
    session_display: SessionDisplay,
    cards: Vec<SwitcherCard>,
}

impl SwitcherGrid {
    /// Builds the stable card order for one Switching Session.
    ///
    /// # Errors
    ///
    /// Returns [`UnknownGridSelection`] when `selected` is not represented by
    /// one of the supplied cards.
    pub fn new(
        session_display: SessionDisplay,
        cards: impl IntoIterator<Item = SwitcherCard>,
        selected: &WindowId,
    ) -> Result<Self, UnknownGridSelection> {
        let mut cards = cards.into_iter().collect::<Vec<_>>();
        update_accessible_positions(&mut cards);
        let mut grid = Self {
            session_display,
            cards,
        };
        grid.select(selected)?;
        Ok(grid)
    }

    #[must_use]
    pub const fn session_display(&self) -> &SessionDisplay {
        &self.session_display
    }

    #[must_use]
    pub fn cards(&self) -> &[SwitcherCard] {
        &self.cards
    }

    /// Changes the selected card without changing MRU Order.
    ///
    /// # Errors
    ///
    /// Returns [`UnknownGridSelection`] when `selected` is not represented in
    /// this Switching Session.
    pub fn select(&mut self, selected: &WindowId) -> Result<(), UnknownGridSelection> {
        if !self.cards.iter().any(|card| card.window == *selected) {
            return Err(UnknownGridSelection(selected.clone()));
        }
        for card in &mut self.cards {
            card.selected = card.window == *selected;
        }
        Ok(())
    }

    pub fn remove(&mut self, window: &WindowId) -> bool {
        let Some(index) = self.cards.iter().position(|card| card.window == *window) else {
            return false;
        };
        self.cards.remove(index);
        update_accessible_positions(&mut self.cards);
        true
    }
}

fn update_accessible_positions(cards: &mut [SwitcherCard]) {
    let set_size = cards.len();
    for (index, card) in cards.iter_mut().enumerate() {
        card.position = index + 1;
        card.set_size = set_size;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorkspaceMoveCapabilities {
    pub legacy: bool,
    pub ext_workspace: bool,
}

impl WorkspaceMoveCapabilities {
    #[must_use]
    pub const fn new(legacy: bool, ext_workspace: bool) -> Self {
        Self {
            legacy,
            ext_workspace,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkspaceMoveCapabilityFailure {
    RequestUnavailable {
        advertised_version: u32,
        required_version: u32,
    },
    NotAdvertised {
        protocol_version: u32,
        legacy_advertised: bool,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ManagementCapabilityContract {
    protocol_version: u32,
    workspace_move: WorkspaceMoveCapabilities,
}

impl ManagementCapabilityContract {
    #[must_use]
    pub const fn new(protocol_version: u32, workspace_move: WorkspaceMoveCapabilities) -> Self {
        Self {
            protocol_version,
            workspace_move,
        }
    }

    /// Chooses the advertised workspace-move request supported by this client.
    ///
    /// # Errors
    ///
    /// Returns a capability failure when the compositor's advertised protocol
    /// version cannot carry the request or its matching capability is absent.
    pub fn verify_workspace_move(&self) -> Result<(), WorkspaceMoveCapabilityFailure> {
        if self.protocol_version < 4 {
            return Err(WorkspaceMoveCapabilityFailure::RequestUnavailable {
                advertised_version: self.protocol_version,
                required_version: 4,
            });
        }

        if self.workspace_move.ext_workspace {
            return Ok(());
        }

        Err(WorkspaceMoveCapabilityFailure::NotAdvertised {
            protocol_version: self.protocol_version,
            legacy_advertised: self.workspace_move.legacy,
        })
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
pub struct WorkspaceMoveRequest {
    pub window: WindowId,
    pub target: WorkspaceId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkspaceMoveRequestFailure {
    TargetAlreadyContainsWindow,
    Capability(WorkspaceMoveCapabilityFailure),
    AlreadyRequested,
}

#[derive(Clone, Debug)]
pub struct WorkspaceMoveVerification {
    window: WindowId,
    original_membership: Vec<WorkspaceId>,
    target: WorkspaceId,
    requested: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VerifiedWorkspaceMove {
    pub window: WindowId,
    pub original_membership: Vec<WorkspaceId>,
    pub resulting_membership: Vec<WorkspaceId>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WorkspaceMoveVerificationFailure {
    NotRequested,
    NotHonored {
        window: WindowId,
        original_membership: Vec<WorkspaceId>,
    },
    TargetNotReached {
        window: WindowId,
        target: WorkspaceId,
        resulting_membership: Vec<WorkspaceId>,
    },
}

impl WorkspaceMoveVerification {
    /// Creates verification state for a move to another workspace.
    ///
    /// # Errors
    ///
    /// Returns [`WorkspaceMoveRequestFailure::TargetAlreadyContainsWindow`]
    /// when the target is already in the Window's original membership.
    pub fn new(
        window: WindowId,
        original_membership: impl IntoIterator<Item = WorkspaceId>,
        target: WorkspaceId,
    ) -> Result<Self, WorkspaceMoveRequestFailure> {
        let original_membership = original_membership.into_iter().collect::<Vec<_>>();
        if original_membership.contains(&target) {
            return Err(WorkspaceMoveRequestFailure::TargetAlreadyContainsWindow);
        }

        Ok(Self {
            window,
            original_membership,
            target,
            requested: false,
        })
    }

    /// Produces the one legal request for this verification.
    ///
    /// # Errors
    ///
    /// Returns a capability failure when no advertised path exists, or
    /// [`WorkspaceMoveRequestFailure::AlreadyRequested`] after the first
    /// request has been produced.
    pub fn request(
        &mut self,
        contract: &ManagementCapabilityContract,
    ) -> Result<WorkspaceMoveRequest, WorkspaceMoveRequestFailure> {
        if self.requested {
            return Err(WorkspaceMoveRequestFailure::AlreadyRequested);
        }

        contract
            .verify_workspace_move()
            .map_err(WorkspaceMoveRequestFailure::Capability)?;
        self.requested = true;

        Ok(WorkspaceMoveRequest {
            window: self.window.clone(),
            target: self.target.clone(),
        })
    }

    /// Verifies the compositor-reported membership after the request.
    ///
    /// # Errors
    ///
    /// Returns a verification failure when no request was produced, membership
    /// stayed unchanged, or the resulting membership omitted the target.
    pub fn verify(
        &self,
        resulting_membership: impl IntoIterator<Item = WorkspaceId>,
    ) -> Result<VerifiedWorkspaceMove, WorkspaceMoveVerificationFailure> {
        if !self.requested {
            return Err(WorkspaceMoveVerificationFailure::NotRequested);
        }

        let mut original_membership = self.original_membership.clone();
        original_membership.sort();
        original_membership.dedup();
        let mut resulting_membership = resulting_membership.into_iter().collect::<Vec<_>>();
        resulting_membership.sort();
        resulting_membership.dedup();

        if resulting_membership == original_membership {
            return Err(WorkspaceMoveVerificationFailure::NotHonored {
                window: self.window.clone(),
                original_membership,
            });
        }
        if !resulting_membership.contains(&self.target) {
            return Err(WorkspaceMoveVerificationFailure::TargetNotReached {
                window: self.window.clone(),
                target: self.target.clone(),
                resulting_membership,
            });
        }

        Ok(VerifiedWorkspaceMove {
            window: self.window.clone(),
            original_membership,
            resulting_membership,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MruHistoryAccuracy {
    WarmUp,
    Accurate,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServiceDiagnostics {
    pub mru_history: MruHistoryAccuracy,
    pub mru_order: Vec<WindowId>,
}

impl fmt::Display for ServiceDiagnostics {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mru_history = match self.mru_history {
            MruHistoryAccuracy::WarmUp => "warm-up",
            MruHistoryAccuracy::Accurate => "accurate",
        };
        write!(
            formatter,
            "service: running\nmru_history: {mru_history}\nwindow_count: {}",
            self.mru_order.len()
        )?;
        if !self.mru_order.is_empty() {
            formatter.write_str("\nmru_order:")?;
            for (position, id) in self.mru_order.iter().enumerate() {
                write!(formatter, "\n  {}. {id}", position + 1)?;
            }
        }
        Ok(())
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
}

#[derive(Clone, Debug, Default)]
pub struct SwitcherService {
    windows: Vec<TrackedWindow>,
    initial_discovery_complete: bool,
    active_session: Option<ActiveSession>,
}

impl SwitcherService {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            windows: Vec::new(),
            initial_discovery_complete: false,
            active_session: None,
        }
    }

    pub const fn complete_initial_discovery(&mut self) {
        self.initial_discovery_complete = true;
    }

    pub fn observe(&mut self, event: WindowEvent) -> Vec<ServiceEffect> {
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

        if let (Some(closed), Some(active_session)) = (closed, self.active_session.as_mut()) {
            let effect = active_session.session.window_closed(&closed);
            let (effects, finished) = active_session.translate_session_effect(effect);
            if finished {
                self.active_session = None;
            }
            return effects;
        }

        Vec::new()
    }

    pub fn invoke(&mut self, request: InvocationRequest) -> Vec<ServiceEffect> {
        let windows = self
            .windows
            .iter()
            .map(|window| window.id.clone())
            .collect::<Vec<_>>();
        let Ok(session) = SwitchingSession::new(
            windows.clone(),
            request.direction,
            request.initial_hold_modifiers,
        ) else {
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
        });
        vec![ServiceEffect::PrepareInvisibleOverlay { selected }]
    }

    pub fn handle(&mut self, event: ServiceEvent) -> Vec<ServiceEffect> {
        let Some(active_session) = self.active_session.as_mut() else {
            return Vec::new();
        };

        match event {
            ServiceEvent::SessionReadinessFailed => {
                let direction = active_session.direction;
                self.active_session = None;
                vec![ServiceEffect::FallbackToStockSwitcher(direction)]
            }
            ServiceEvent::SessionReady => {
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
                let effect = active_session
                    .session
                    .handle(SwitchingEvent::HoldModifiersChanged(modifiers));
                let (effects, finished) = active_session.translate_session_effect(effect);
                if finished {
                    self.active_session = None;
                }
                effects
            }
            ServiceEvent::Switching(event) => {
                let effect = active_session.session.handle(event);
                let (effects, finished) = active_session.translate_session_effect(effect);
                if finished {
                    self.active_session = None;
                }
                effects
            }
            ServiceEvent::Invocation(direction) => {
                let effect = active_session.session.move_selection(direction);
                let (effects, finished) = active_session.translate_session_effect(effect);
                if finished {
                    self.active_session = None;
                }
                effects
            }
        }
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
        }
    }
}
