# Default to All Workspaces

The packaged `cosmic-comp 1.0.0` advertises
`zcosmic_toplevel_info_v1` version 3 and `ext_workspace_manager_v1`
version 1, but does not emit the initial per-Window output/workspace
memberships or the atomic toplevel-info `done` event. Without that
authoritative mapping, an external client cannot derive the Visible Workspace
Set correctly.

The COSMIC Window Switcher therefore defaults to **All Workspaces**: every
foreign application Window participates in one global MRU Order. Missing
workspace membership remains diagnostic but is not required for Session
Readiness in this scope. The stricter **Visible Workspaces** implementation is
retained and remains capability-gated for future use.

## Considered Options

- Continuing to delegate every invocation to the stock switcher was safe but
  made the custom switcher unusable on the target COSMIC release.
- Guessing membership from workspace configuration or Window metadata could
  show an incorrect set and would duplicate or contradict COSMIC Workspace
  Policy.
- Patching or forking `cosmic-comp` would provide the intended protocol data
  but would delay use, introduce system-level maintenance, and depend on an
  external project that may already be correcting the issue.
- All Workspaces requires no guessed membership, works with the available
  foreign-toplevel and management protocols, and matches the chosen behavior of
  switching across workspace boundaries.

## Consequences

Windows on inactive workspaces intentionally appear in the Switcher Grid.
Activating one asks COSMIC to unminimize it, activate its workspace, and focus
it. The overlay prefers the initially focused Window's output; when COSMIC
omits that output membership, it deterministically uses an output assigned to
a workspace group. Visible Workspaces must not become the default until its
membership snapshot is verified live or the user explicitly selects that
scope.
