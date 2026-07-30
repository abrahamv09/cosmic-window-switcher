# 02 — Maintain observable MRU Order

**What to build:** A resident Switcher Service that observes Window activation throughout the COSMIC Session and exposes an understandable current MRU Order through diagnostics. A user can verify that the current Window is first, recently focused Windows move forward predictably, and unknown history after restart is represented as MRU Warm-up rather than arbitrary recovered history.

**Blocked by:** 01 — Prove a two-Window COSMIC switch.

**Status:** resolved

- [x] The service starts in a COSMIC Session, owns a single user-session D-Bus name, and remains resident without opening a visible Window.
- [x] Observed activation transitions place the current Window first and preserve correct recency for all known Windows.
- [x] Closed Windows disappear and newly discovered Windows receive deterministic initial placement.
- [x] Restarting with pre-existing Windows enters MRU Warm-up: current first, unknown survivors in stable discovery order.
- [x] `status` distinguishes accurate MRU history from MRU Warm-up without exposing Window titles by default.
- [x] The idle service captures no pixels and has no sustained thumbnail-refresh work.
- [x] Service-scenario tests cover focus sequences, duplicate events, closure, restart, and identity reuse.

## Comments

- Implemented in commits `10e86c1` and `b479c66`.
- Added `service` and `status` to the Command Surface with a single versioned user-session D-Bus name.
- Preserved activation ordering even when compositor identity metadata arrives late.
- Verified formatting, Clippy with warnings denied, service-scenario tests, the full test suite, and a release build.
- Standards and specification review found no remaining requirements gap.
