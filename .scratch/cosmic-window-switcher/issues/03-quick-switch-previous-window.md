# 03 — Quick-switch to the previous Window

**What to build:** A complete invisible quick-switch path: invoking forward or reverse through D-Bus selects the correct Window from MRU Order, observes the initial modifier lifecycle, and activates without flashing an overlay when the chord is released before the reveal delay.

**Blocked by:** 02 — Maintain observable MRU Order.

**Status:** resolved

- [x] `invoke next` selects the second MRU item and `invoke previous` selects the final MRU item.
- [x] A quick hold-modifier release activates the selection without revealing visible overlay content.
- [x] One Eligible Window is a no-op that preserves focus.
- [x] Zero surviving candidates cancel safely.
- [x] A missing or unresponsive service receives one bounded recovery attempt.
- [x] Failed Session Readiness invokes the stock COSMIC launcher directly in the requested direction without recursive semantic-action dispatch.
- [x] D-Bus messages and default diagnostics contain no Window pixels or titles.
- [x] Service-scenario and isolated session-bus tests cover forward, reverse, one-item, timeout, restart, and fallback behavior.

## Comments

- Implemented in commit `ed56508`.
- Added `invoke next|previous` over the versioned user-session D-Bus interface with an eventfd wakeup, bounded 250 ms calls, one recovery attempt, and direct direction-preserving stock-launcher fallback.
- Added the transparent exclusive-keyboard quick-switch runtime, initial Alt/Ctrl/Super lifecycle handling, forward/reverse MRU selection, Enter/Escape handling, one-Window no-op, closed-Window advancement, and zero-Window cancellation.
- The ticket-3 runtime delegates to the stock switcher when the reveal delay expires because the icon/title grid is owned by ticket 4; it never leaves an invisible held session open.
- Isolated session-bus tests cover absent, D-Bus-activated, and unresponsive services without carrying Window titles or pixels. Service-scenario tests cover selection, quick release, readiness failure, closure, and repeated directions.
- Verified formatting, strict Clippy, the full all-target/all-feature test suite, a release build, a live service/status smoke test, and idle blocking behavior.
- Two-axis review found one domain-vocabulary violation and lifecycle gaps for selected-Window closure and Latch Mode. These were corrected; presentation-readiness gaps delegate safely to stock pending ticket 4.
