# 07 — Respect COSMIC Window, workspace, and display state

**What to build:** Make the Switcher Grid reflect the desktop the user actually configured. It includes exactly the Eligible Windows from the Visible Workspace Set across outputs, treats Native Wayland and XWayland Windows consistently, restores minimized selections, and places one overlay on the correct Session Display.

**Blocked by:** 06 — Navigate the continuous grid with keyboard and mouse.

**Status:** ready-for-agent

- [ ] Live workspace groups, outputs, active state, and Window membership determine eligibility without copying a competing workspace-mode preference.
- [ ] Spanning workspaces include their visible Windows across all displays.
- [ ] Separate-display workspaces include the active workspace from each display.
- [ ] Minimized Windows remain eligible and restore when activated.
- [ ] Independently exposed dialogs and utility Windows are eligible; panels, docks, menus, notifications, and overlays are excluded.
- [ ] Native Wayland and compositor-managed XWayland Windows share one MRU Order and activation behavior.
- [ ] The sole overlay appears on the display containing the initially focused Window and preserves fullscreen state.
- [ ] Runtime workspace-policy changes affect the next Switching Session without restarting the service.
- [ ] Live and service-scenario tests cover available multi-monitor, workspace, minimized, dialog, shell-surface, fullscreen, and mixed Window-type cases.

