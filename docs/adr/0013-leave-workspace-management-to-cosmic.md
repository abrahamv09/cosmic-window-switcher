# Leave workspace management to COSMIC

COSMIC Window Switcher includes application Windows from All Workspaces and
uses normal activation to follow a selected Window to its existing workspace.
It does not show a workspace view, expose workspace targets, or relocate
Windows: COSMIC's existing workspace interface owns those tasks. This keeps the
product focused on switching and removes its dependency on COSMIC's unreliable
external workspace-move capability.

## Consequences

Workspace-move verification and drag-to-workspace are not product requirements.
Tickets 11 and 12 are closed as `wontfix`, and the release no longer waits for
either an upstream compositor change or a workspace-drag implementation.
