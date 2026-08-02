# 14 — Install and validate the v1 release

**What to build:** A verifiable Pop!_OS 24.04 `amd64` release that users can download, authenticate, install, explicitly enable, upgrade, disable, and remove while always retaining a usable stock COSMIC switcher.

**Blocked by:** 07 — Respect COSMIC Window, workspace, and display state; 08 — Configure an accessible bilingual experience; 09 — Stay responsive with GPU-native capture; 10 — Protect previews across interruptions and failures; 13 — Enable, disable, and recover safely.

**Status:** ready-for-human

**Progress:** release implementation complete; physical two-machine validation
and signed GitHub publication remain

- [x] The Debian package declares known minimum runtime dependencies and installs the executable, metadata, icon, localization, service integration, defaults, license, and user documentation.
- [x] Clean install, explicit enablement, upgrade, disable, removal, and purge behave as documented without damaging user-managed shortcuts.
- [x] Removing or disabling the package leaves the stock COSMIC switcher immediately usable.
- [x] Continuous integration produces the release package, checksum manifest, and artifacts required for maintainer signing.
- [x] Installation and recovery documentation covers unsupported sessions, capability failure, `status`, `doctor`, and manual COSMIC shortcut configuration.
- [ ] The development laptop passes the complete automated and manual release matrix, including performance targets.
- [ ] The MSI Aegis ZS2 passes independent install, GPU capture, XWayland, performance, upgrade, disable, and uninstall validation.
- [ ] The GitHub Release contains a maintainer signature and verifiable checksum and makes no unsupported architecture or desktop-session claims.
- [x] No v2 controls, apt repository, automatic updater, telemetry, crash uploader, sandboxed package, or persisted thumbnail data are included.

## Comments

- Added Debian packaging for Pop!_OS 24.04 `amd64` with audited minimum
  `cosmic-comp`, `cosmic-launcher`, and `cosmic-settings` versions plus the
  executable's generated shared-library dependencies. The payload includes the
  stable desktop metadata, original scalable icon, AppStream metadata, embedded
  and installed English/Spanish resources, versioned `cosmic-config` defaults,
  D-Bus and systemd-user integration, GPL documentation, and user guides.
- Package installation and upgrade are inert. The artifact contract runs a
  real `.deb` through an isolated `dpkg` database for clean install,
  same-version upgrade, remove, purge, and failed shortcut-restoration paths.
  Removal restores only app-owned semantic commands; if restoration fails,
  `prerm` aborts and retains the fallback-capable executable.
- CI builds and checks the `.deb`, debug-symbol `.ddeb`, `.buildinfo`,
  `.changes`, and `SHA256SUMS`. A version-tag workflow imports the maintainer
  key from the repository secret, creates and verifies `SHA256SUMS.asc`, and
  publishes only the documented Pop!_OS 24.04 COSMIC Wayland `amd64` artifacts.
- Added authentication, install, explicit enablement, manual COSMIC shortcut,
  diagnostics, capability fallback, upgrade, disable, removal, purge, release
  notes, and two-machine validation documentation. The validation record keeps
  unperformed physical checks visibly `Pending`.
- Automated verification passed formatting, all-target type checking, strict
  all-target/all-feature Clippy, all 122 tests, AppStream validation, package
  and maintainer-script syntax, isolated lifecycle checks, and release checksum
  verification. The final local `.deb` SHA-256 is
  `f2901604007ba5610fd9ecbebd558cf56b1eb47e7e0088e960d77a83368f24ef`.
- The two-axis review identified ambiguous domain wording, an undocumented
  AppStream metadata-license exception, incomplete custom-XDG uninstall
  recovery, a package test that logged rather than executed uninstall cleanup,
  a same-version upgrade simulation, and publication without a physical-test
  gate. Enablement now writes an atomic fixed-location locator for the user's
  actual XDG config/state paths; package removal executes the packaged
  ownership-safe disable for any non-root UID and aborts on missing, corrupt, or
  failed recovery. The artifact test verifies restored custom-XDG shortcut
  values across a real lower-version upgrade. Domain metadata and licensing are
  explicit, and tag publication requires a checksum-matched two-machine
  attestation with retained evidence.
- Human handoff: run and record the complete manual/performance matrix on the
  development laptop and MSI Aegis ZS2, configure `RELEASE_GPG_PRIVATE_KEY`,
  publish the version tag, download and independently verify the signed Release,
  then resolve this ticket only if all three remaining checkboxes pass.
