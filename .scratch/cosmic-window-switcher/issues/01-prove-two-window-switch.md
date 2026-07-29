# 01 — Prove a two-Window COSMIC switch

**What to build:** A minimal, verifiable COSMIC Window Switcher path that discovers two real Windows, obtains one memory-only thumbnail through shared memory, receives the switching keys and hold-modifier release, and activates the selected Window. This tracer bullet also establishes the build, license, application identity, formatting, linting, and automated-test foundation needed by later tickets.

**Blocked by:** None — can start immediately.

**Status:** resolved

- [x] A clean checkout builds and tests one GPL-3.0-only Rust executable carrying the approved application identity.
- [x] The probe enumerates at least two compositor-managed Windows and distinguishes their opaque identities, titles, and application identities.
- [x] The probe obtains one exact-size SHM frame without writing Window pixels to disk.
- [x] An initially transparent exclusive-keyboard overlay observes Tab, Escape, and release of the initial hold modifier.
- [x] Releasing the modifier activates the chosen Window exactly once; Escape leaves the original Window focused.
- [x] The same probe succeeds with one Native Wayland Window and one representative XWayland Window, or records the XWayland capture limitation without dropping it from discovery.
- [x] Formatting, linting, unit tests, and a release build run in continuous integration.

## Comments

- Implemented in commit `6cc5b80`.
- Verified formatting, Clippy with warnings denied, all tests, and a release build.
- Live COSMIC validation discovered Native Wayland and XWayland Windows and obtained exact-size memory-only SHM frames for both.
- Standards review found no hard violations; specification review found no remaining requirements gap.
