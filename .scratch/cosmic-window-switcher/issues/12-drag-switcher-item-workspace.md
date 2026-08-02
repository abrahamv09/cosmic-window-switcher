# 12 — Drag a Switcher Item to another workspace

**What to build:** COSMIC-style pointer workspace movement within an active Switching Session. A deliberate drag reveals correctly ordered workspace targets, a valid drop moves the Window, and cancellation or invalid targets have no compositor side effects.

**Blocked by:** 07 — Respect COSMIC Window, workspace, and display state; 11 — Move one Window through a verified workspace capability.

**Status:** wontfix

- [ ] Only a deliberate drag gesture from a Switcher Item begins workspace movement.
- [ ] Targets follow live COSMIC workspace groups, coordinates, orientation, and separate/spanning display policy.
- [ ] Dropping on a valid target issues one verified move request and reflects the compositor-confirmed result.
- [ ] A Window moved outside the Visible Workspace Set leaves the current Session Window Set without ending the Switching Session.
- [ ] Escape, invalid drop, Window closure, output change, and Session Deactivation cancel without moving the Window.
- [ ] No hover or incidental pointer motion can move a Window.
- [ ] The UI does not expose a target when the verified move capability is unavailable.
- [ ] The v1 pointer-only accessibility limitation and planned v2 keyboard picker are documented.

## Answer

The switcher does not duplicate COSMIC's workspace interface. It displays and
activates application Windows across all workspaces but does not expose
workspace targets or relocate Windows. This ticket was removed from scope by
ADR-0013.
