# 01 — Prove a two-Window COSMIC switch

**What to build:** A minimal, verifiable COSMIC Window Switcher path that discovers two real Windows, obtains one memory-only thumbnail through shared memory, receives the switching keys and hold-modifier release, and activates the selected Window. This tracer bullet also establishes the build, license, application identity, formatting, linting, and automated-test foundation needed by later tickets.

**Blocked by:** None — can start immediately.

**Status:** ready-for-agent

- [ ] A clean checkout builds and tests one GPL-3.0-only Rust executable carrying the approved application identity.
- [ ] The probe enumerates at least two compositor-managed Windows and distinguishes their opaque identities, titles, and application identities.
- [ ] The probe obtains one exact-size SHM frame without writing Window pixels to disk.
- [ ] An initially transparent exclusive-keyboard overlay observes Tab, Escape, and release of the initial hold modifier.
- [ ] Releasing the modifier activates the chosen Window exactly once; Escape leaves the original Window focused.
- [ ] The same probe succeeds with one Native Wayland Window and one representative XWayland Window, or records the XWayland capture limitation without dropping it from discovery.
- [ ] Formatting, linting, unit tests, and a release build run in continuous integration.

