# 14 — Install and validate the v1 release

**What to build:** A verifiable Pop!_OS 24.04 `amd64` release that users can download, authenticate, install, explicitly enable, upgrade, disable, and remove while always retaining a usable stock COSMIC switcher.

**Blocked by:** 07 — Respect COSMIC Window, workspace, and display state; 08 — Configure an accessible bilingual experience; 09 — Stay responsive with GPU-native capture; 10 — Protect previews across interruptions and failures; 12 — Drag a Switcher Item to another workspace; 13 — Enable, disable, and recover safely.

**Status:** ready-for-agent

- [ ] The Debian package declares known minimum runtime dependencies and installs the executable, metadata, icon, localization, service integration, defaults, license, and user documentation.
- [ ] Clean install, explicit enablement, upgrade, disable, removal, and purge behave as documented without damaging user-managed shortcuts.
- [ ] Removing or disabling the package leaves the stock COSMIC switcher immediately usable.
- [ ] Continuous integration produces the release package, checksum manifest, and artifacts required for maintainer signing.
- [ ] Installation and recovery documentation covers unsupported sessions, capability failure, `status`, `doctor`, and manual COSMIC shortcut configuration.
- [ ] The development laptop passes the complete automated and manual release matrix, including performance targets.
- [ ] The MSI Aegis ZS2 passes independent install, GPU capture, XWayland, performance, upgrade, disable, and uninstall validation.
- [ ] The GitHub Release contains a maintainer signature and verifiable checksum and makes no unsupported architecture or desktop-session claims.
- [ ] No v2 controls, apt repository, automatic updater, telemetry, crash uploader, sandboxed package, or persisted thumbnail data are included.

