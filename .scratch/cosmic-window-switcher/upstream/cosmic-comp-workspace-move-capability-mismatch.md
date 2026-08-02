# `cosmic-comp` advertises the ignored workspace-move request, not the implemented request

Prepared: 2026-07-29

Revalidated: 2026-08-02

## Environment

- Pop!_OS 24.04 COSMIC Wayland session
- `cosmic-comp` package `0.1~1785277801~24.04~ffeda33`
- `cosmic-comp 1.0.0` (`ffeda3375a7e60ace6ae64b19432f1f0c1fc1034`)
- One output (`eDP-1`) with two ext-workspaces

## Reproduction

Run an unsandboxed Wayland client that binds:

- `zcosmic_toplevel_manager_v1` version 4;
- `zcosmic_toplevel_info_v1` version 3;
- `ext_workspace_manager_v1` version 1.

The repository's reproducer is:

```sh
cargo run --release -- probe-workspace-move
```

It records this runtime capability snapshot:

```text
Advertised protocols: zcosmic_toplevel_manager_v1=v4 (bound v4), zcosmic_toplevel_info_v1=v3, ext_workspace_manager_v1=v1.
Advertised Window management capabilities=[1, 2, 3, 4, 6].
```

Then request a move using ids/selectors printed by the inventory:

```sh
cargo run --release -- probe-workspace-move \
  --window <window-id> \
  --workspace <workspace-selector>
```

The reproducer exits before sending a Wayland move request:

```text
Error: failed workspace-move capability: NotAdvertised {
    protocol_version: 4,
    legacy_advertised: true,
}
```

## Expected

The compositor should advertise the capability for the request it honors:

- capability 8 (`move_to_ext_workspace`) with manager version 4; or
- capability 6 (`move_to_workspace`) only if its legacy request is implemented.

A capability-respecting client cannot send `move_to_ext_workspace` while
capability 8 is absent.

## First-party source evidence

The protocol assigns value 6 to `move_to_workspace`, value 8 to
`move_to_ext_workspace`, and directs clients to hide or disable functionality
whose capability is absent:

<https://github.com/pop-os/cosmic-protocols/blob/main/unstable/cosmic-toplevel-management-unstable-v1.xml>

Current `cosmic-comp` setup advertises `ManagementCapabilities::MoveToWorkspace`
and does not include `MoveToExtWorkspace`:

<https://github.com/pop-os/cosmic-comp/blob/master/src/state.rs>

Current request dispatch ignores `MoveToWorkspace { .. }` but handles
`MoveToExtWorkspace` by calling `state.move_to_workspace(...)`:

<https://github.com/pop-os/cosmic-comp/blob/master/src/wayland/protocols/toplevel_management.rs>

## Minimal correction

In the toplevel-management capability list, replace
`ManagementCapabilities::MoveToWorkspace` with
`ManagementCapabilities::MoveToExtWorkspace`. Keep the legacy capability only
if the legacy request handler is implemented as well.

After that change, rerun the reproducer and verify that it issues one
`move_to_ext_workspace` request and observes the chosen Window enter the target
ext-workspace.

## Secondary runtime observation

On the packaged build above, `zcosmic_toplevel_info_v1` version 3 also did not
emit initial `ext_workspace_enter` membership or its atomic `done` event for
the two discovered Windows. The reproducer reports this separately and refuses
to claim a successful move without resulting membership. This may need a
separate upstream report if it remains after the capability advertisement is
corrected.

## Revalidation

On 2026-08-02, rerunning the inventory against the installed `ffeda33` build
reproduced the same capability list, empty Window output/workspace membership,
and missing atomic toplevel-info `done` event. The probe did not issue a move
request.

The Pop!_OS package candidate was
`0.1~1785355703~24.04~091583a`; that source revision is already covered by the
evidence above and retains the mismatch. Upstream `master` at
`d3ffa814941f6294864d5ecdc9796f818ddb1ac8` also still advertises
`MoveToWorkspace`, ignores `MoveToWorkspace`, and handles
`MoveToExtWorkspace`. A GitHub issue and pull-request search for the exact
protocol symbols found no existing tracked correction.
