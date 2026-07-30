// SPDX-License-Identifier: GPL-3.0-only

use cosmic_window_switcher::{
    ManagementCapabilityContract, VerifiedWorkspaceMove, WindowId, WorkspaceId,
    WorkspaceMoveCapabilities, WorkspaceMoveCapabilityFailure, WorkspaceMoveRequest,
    WorkspaceMoveRequestFailure, WorkspaceMoveVerification, WorkspaceMoveVerificationFailure,
};

#[test]
fn unadvertised_workspace_move_capability_never_produces_a_request() {
    let contract =
        ManagementCapabilityContract::new(4, WorkspaceMoveCapabilities::new(true, false));

    assert_eq!(
        contract.verify_workspace_move(),
        Err(WorkspaceMoveCapabilityFailure::NotAdvertised {
            protocol_version: 4,
            legacy_advertised: true,
        })
    );
}

#[test]
fn workspace_move_requires_the_protocol_version_that_defines_its_request() {
    let contract =
        ManagementCapabilityContract::new(3, WorkspaceMoveCapabilities::new(false, true));

    assert_eq!(
        contract.verify_workspace_move(),
        Err(WorkspaceMoveCapabilityFailure::RequestUnavailable {
            advertised_version: 3,
            required_version: 4,
        })
    );
}

#[test]
fn supported_workspace_move_produces_exactly_one_request() {
    let contract =
        ManagementCapabilityContract::new(4, WorkspaceMoveCapabilities::new(false, true));
    let mut verification = WorkspaceMoveVerification::new(
        WindowId::from("window-7"),
        [WorkspaceId::from("workspace-1")],
        WorkspaceId::from("workspace-2"),
    )
    .expect("the target is another workspace");

    assert_eq!(
        verification.request(&contract),
        Ok(WorkspaceMoveRequest {
            window: WindowId::from("window-7"),
            target: WorkspaceId::from("workspace-2"),
        })
    );
    assert_eq!(
        verification.request(&contract),
        Err(WorkspaceMoveRequestFailure::AlreadyRequested)
    );
}

#[test]
fn compositor_membership_change_verifies_the_workspace_move() {
    let contract =
        ManagementCapabilityContract::new(4, WorkspaceMoveCapabilities::new(false, true));
    let mut verification = WorkspaceMoveVerification::new(
        WindowId::from("window-7"),
        [WorkspaceId::from("workspace-1")],
        WorkspaceId::from("workspace-2"),
    )
    .expect("the target is another workspace");
    verification
        .request(&contract)
        .expect("the capability is advertised");

    assert_eq!(
        verification.verify([WorkspaceId::from("workspace-2")]),
        Ok(VerifiedWorkspaceMove {
            window: WindowId::from("window-7"),
            original_membership: vec![WorkspaceId::from("workspace-1")],
            resulting_membership: vec![WorkspaceId::from("workspace-2")],
        })
    );
}

#[test]
fn unchanged_membership_reports_an_advertised_but_unhonored_capability() {
    let contract =
        ManagementCapabilityContract::new(4, WorkspaceMoveCapabilities::new(false, true));
    let mut verification = WorkspaceMoveVerification::new(
        WindowId::from("window-7"),
        [WorkspaceId::from("workspace-1")],
        WorkspaceId::from("workspace-2"),
    )
    .expect("the target is another workspace");
    verification
        .request(&contract)
        .expect("the capability is advertised");

    assert_eq!(
        verification.verify([WorkspaceId::from("workspace-1")]),
        Err(WorkspaceMoveVerificationFailure::NotHonored {
            window: WindowId::from("window-7"),
            original_membership: vec![WorkspaceId::from("workspace-1")],
        })
    );
}
