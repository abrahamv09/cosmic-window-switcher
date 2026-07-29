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
