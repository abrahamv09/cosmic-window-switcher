# Test Matrix

## Development baseline

- Pop!_OS 24.04 LTS
- COSMIC Wayland session
- `amd64`
- Intel Core Ultra 9 288V, 8 cores
- Intel Lunar Lake graphics using the `xe` driver
- 30 GiB RAM
- 2880×1800 display at 120 Hz and 175% scale

This machine is the reference for the v1 performance targets:

- No capture work or sustained CPU usage while the service is idle.
- Selection feedback within one display frame.
- Overlay ready within 50 ms after the configured reveal delay.
- Smooth 30 FPS Live Thumbnails with 10 open Windows without delaying input.
- Input responsiveness takes priority over thumbnail freshness under overload; the selected item remains freshest and all others continue fair round-robin updates at full resolution.

## Secondary production-style environment

- MSI Aegis ZS2
- Pop!_OS 24.04 LTS
- COSMIC Wayland session
- Exact CPU, GPU, memory, display, scale, and refresh configuration: capture when the machine is available.

Use this machine for independent package installation, upgrade, shortcut restoration, GPU capture, performance, and release-candidate validation.

## Version 1 release gates

- Automated tests cover MRU Order, deterministic MRU Warm-up, eligibility, Session Window Set stability, immutable Session Preferences, continuous-grid row wrapping, selection reveal, visible-row capture suspension and resumption, Window closure, configuration migration, and safe shortcut backup/restore.
- Protocol-level integration tests run wherever they do not require physical compositor hardware, including atomic Session Readiness failure, direction-preserving stock-switcher fallback, and Session Deactivation cleanup.
- Compatibility tests exercise the minimum Capability Contract, compatible newer protocol versions, missing required capabilities, and ignored optional capabilities.
- Both hardware environments pass a manual release checklist covering Hold Mode, Latch Mode, forward and reverse cycling, modifier release, DMA-BUF capture, forced shared-memory fallback, Native Wayland Windows, XWayland Windows, workspace modes, multiple displays where available, fullscreen, minimization, scaling, install, upgrade, stock-switcher fallback, disable, and uninstall restoration.
- Workspace movement cannot pass its release gate until the compositor advertises and honors the same supported request in an end-to-end runtime probe.
- Launching in GNOME, Ubuntu, or another non-COSMIC session exits cleanly with an unsupported-environment diagnostic and never installs or changes shortcuts there.

## Live Thumbnail contract

Run `cargo run --release -- probe --live-thumbnails` and the resident Switcher
Service on the development COSMIC Session. The probe output may include opaque
Window ids, application identities, exact dimensions, allocation byte counts,
SHM formats, and frame counts; it must never include pixels or titles unless
title output is explicitly requested. Press Escape after exercising the cases
below so the probe also verifies explicit session-stop cleanup.

| Contract case | Setup and observation |
| --- | --- |
| Native Wayland Window | Open a Native Wayland terminal, invoke the switcher, and verify an exact-size SHM frame and an uncropped card. |
| Representative XWayland Window | Open the release-matrix XWayland client beside the Native Wayland Window and verify that it remains in MRU Order whether capture succeeds or degrades. |
| Changed content | Animate or scroll one visible Window and verify damage-driven updates no faster than the selected Refresh Ceiling. |
| Unchanged content | Leave another visible Window static and verify that no duplicate frame is requested while the compositor waits for damage. |
| Minimized Window | Minimize an Eligible Window before invocation and verify its last frame or icon-and-title fallback remains switchable and restores on activation. |
| Per-Window failure | Exercise protected content or terminate one capture source and verify only its Switcher Item degrades. |
| Session stop | Cancel and commit separate Switching Sessions, then close a Window during another session; verify all corresponding capture sessions and anonymous SHM allocations are released. |

The deterministic `live_thumbnail_capture` test exercises the same public
capture contract with fake damage, time, constraints, failure, viewport, Window
closure, and session-stop events. It is the repeatable regression suite; the
live matrix verifies compositor interoperability.

## Live Window and workspace-state contract

Run `cargo run --release -- probe-workspace-move` before each scenario to record
the compositor's live workspace groups, output assignments, active workspace
state, and Window membership. Run the resident service, invoke the Switcher
Grid, and compare its Window set and display placement with that inventory.
Change COSMIC's workspace policy while the service remains running and repeat
the invocation; an already open Switching Session remains stable, while the
next one uses the new state.

| Contract case | Setup and observation |
| --- | --- |
| Spanning workspaces | With two displays in one workspace group, place a Window on each display and verify both are present in one MRU Order. |
| Separate-display workspaces | Use one workspace group per display, activate a different workspace on each, and verify Windows from both active workspaces are present while inactive-workspace Windows are absent. |
| Runtime policy change | Switch between spanning and separate-display policy without restarting the service and verify the next Switching Session follows the new groups and active workspaces. |
| Minimized Window | Minimize an Eligible Window, select it, and verify COSMIC restores and focuses it. |
| Dialog and utility Window | Open independently exposed dialog and utility Windows and verify each has its own Switcher Item. |
| Shell surfaces | Verify COSMIC panels, docks, menus, notifications, and overlays never appear as Switcher Items. |
| Mixed Window types | Open one Native Wayland Window and one compositor-managed XWayland Window and verify both use the same MRU Order and activation behavior. |
| Fullscreen Window | Invoke over a fullscreen Window, verify exactly one overlay appears above it on that Window's display, then cancel and confirm fullscreen state was preserved. |
| Multi-output placement | Focus a Window on each available display in turn and verify the sole overlay follows the initially focused Window without duplication. |

The deterministic `workspace_eligibility` and `service_scenario` tests exercise
the same public snapshot and activation behavior with fake spanning and
separate-display groups, active-state changes, minimized/fullscreen state,
dialogs, utility Windows, shell surfaces, and mixed Window identities.
