# 02 — Maintain observable MRU Order

**What to build:** A resident Switcher Service that observes Window activation throughout the COSMIC Session and exposes an understandable current MRU Order through diagnostics. A user can verify that the current Window is first, recently focused Windows move forward predictably, and unknown history after restart is represented as MRU Warm-up rather than arbitrary recovered history.

**Blocked by:** 01 — Prove a two-Window COSMIC switch.

**Status:** ready-for-agent

- [ ] The service starts in a COSMIC Session, owns a single user-session D-Bus name, and remains resident without opening a visible Window.
- [ ] Observed activation transitions place the current Window first and preserve correct recency for all known Windows.
- [ ] Closed Windows disappear and newly discovered Windows receive deterministic initial placement.
- [ ] Restarting with pre-existing Windows enters MRU Warm-up: current first, unknown survivors in stable discovery order.
- [ ] `status` distinguishes accurate MRU history from MRU Warm-up without exposing Window titles by default.
- [ ] The idle service captures no pixels and has no sustained thumbnail-refresh work.
- [ ] Service-scenario tests cover focus sequences, duplicate events, closure, restart, and identity reuse.

