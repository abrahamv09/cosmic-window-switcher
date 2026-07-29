# 03 — Quick-switch to the previous Window

**What to build:** A complete invisible quick-switch path: invoking forward or reverse through D-Bus selects the correct Window from MRU Order, observes the initial modifier lifecycle, and activates without flashing an overlay when the chord is released before the reveal delay.

**Blocked by:** 02 — Maintain observable MRU Order.

**Status:** ready-for-agent

- [ ] `invoke next` selects the second MRU item and `invoke previous` selects the final MRU item.
- [ ] A quick hold-modifier release activates the selection without revealing visible overlay content.
- [ ] One Eligible Window is a no-op that preserves focus.
- [ ] Zero surviving candidates cancel safely.
- [ ] A missing or unresponsive service receives one bounded recovery attempt.
- [ ] Failed Session Readiness invokes the stock COSMIC launcher directly in the requested direction without recursive semantic-action dispatch.
- [ ] D-Bus messages and default diagnostics contain no Window pixels or titles.
- [ ] Service-scenario and isolated session-bus tests cover forward, reverse, one-item, timeout, restart, and fallback behavior.

