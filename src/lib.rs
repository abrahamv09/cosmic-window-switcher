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
