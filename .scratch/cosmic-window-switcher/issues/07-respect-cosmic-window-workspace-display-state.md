# 07 — Respect COSMIC Window, workspace, and display state

**What to build:** Make the Switcher Grid reflect the desktop the user actually configured. It includes exactly the Eligible Windows from the Visible Workspace Set across outputs, treats Native Wayland and XWayland Windows consistently, restores minimized selections, and places one overlay on the correct Session Display.

**Blocked by:** 06 — Navigate the continuous grid with keyboard and mouse.

**Status:** ready-for-human

**Progress:** partially completed

**Waiting on:** Upstream COSMIC compositor support for the advertised initial
toplevel output/workspace membership snapshot and atomic `done` event.

- [x] Live workspace groups, outputs, active state, and Window membership determine eligibility without copying a competing workspace-mode preference.
- [x] Spanning workspaces include their visible Windows across all displays.
- [x] Separate-display workspaces include the active workspace from each display.
- [x] Minimized Windows remain eligible and restore when activated.
- [x] Independently exposed dialogs and utility Windows are eligible; panels, docks, menus, notifications, and overlays are excluded.
- [x] Native Wayland and compositor-managed XWayland Windows share one MRU Order and activation behavior.
- [x] The sole overlay appears on the display containing the initially focused Window and preserves fullscreen state.
- [x] Runtime COSMIC Workspace Policy changes affect the next Switching Session without restarting the service.
- [ ] Live and service-scenario tests cover available multi-monitor, workspace, minimized, dialog, shell-surface, fullscreen, and mixed Window-type cases.

## Comments

- Implemented the workspace-aware invocation snapshot and nonfatal capability
  reporting in commits `5a085ac`, `226e7d1`, `1a4e30b`, and `8d77e82`. The
  client derives the Visible Workspace Set from live workspace groups, output
  assignments, active/hidden state, and committed Window membership; it never
  copies COSMIC Workspace Policy into Switcher Preferences.
- Added deterministic eligibility and service scenarios for spanning and
  separate-display groups, policy changes between sessions, minimized and
  fullscreen Windows, independently exposed dialogs/utilities, mixed Window
  identities, authoritative Session Display selection, and direction-preserving
  stock fallback.
- Live validation on 2026-07-30 reproduced the upstream blocker on packaged
  `cosmic-comp 1.0.0` (`ffeda3375a7e60ace6ae64b19432f1f0c1fc1034`):
  toplevel-info v3 and ext-workspace v1 were advertised, but neither initial
  Window output/workspace membership nor the atomic toplevel-info `done` event
  arrived.
- The service now remains resident under that defect, retains MRU Warm-up, and
  reports `workspace_eligibility: unavailable`; forward and reverse Invocation
  Requests delegate to the stock switcher rather than guessing an incorrect
  Window set.
- The successful live multi-monitor/minimized/dialog/fullscreen/mixed-client
  matrix remains incomplete until the compositor supplies the advertised
  snapshot. See
  `.scratch/cosmic-window-switcher/upstream/cosmic-comp-workspace-move-capability-mismatch.md`.
