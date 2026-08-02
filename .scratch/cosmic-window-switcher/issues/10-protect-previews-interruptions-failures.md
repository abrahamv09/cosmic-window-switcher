# 10 — Protect previews across interruptions and failures

**What to build:** Make privacy and focus safety hold when the desktop changes unexpectedly. Lock, suspend, user switch, output loss, compositor loss, Window closure, and session shutdown cancel cleanly, release every preview resource, and never resurrect a stale overlay after resume.

**Blocked by:** 07 — Respect COSMIC Window, workspace, and display state; 09 — Stay responsive with GPU-native capture.

**Status:** resolved

- [x] Screen lock, suspend, user switch, and COSMIC session shutdown cancel without activating another Window.
- [x] Session Deactivation destroys overlay surfaces, capture sessions, imported buffers, and last-frame references.
- [x] MRU observation pauses outside an active unlocked COSMIC Session and rebuilds safely after resume.
- [x] Output removal or compositor disconnect during switching cannot leave an input-grabbing invisible surface.
- [x] A selected Window closing advances selection when possible and cancels safely when no candidates survive.
- [x] Default journal output, D-Bus traffic, crash diagnostics, and `doctor` output contain no Window pixels or titles.
- [x] No telemetry or automatic crash upload is introduced.
- [x] Lifecycle and service-scenario tests verify cleanup after every interruption point and backend state.

## Comments

- Live validation on 2026-07-30 observed the resident service exit after a
  successful cross-workspace activation because the Wayland read following
  poll readiness returned `EAGAIN` (`Resource temporarily unavailable`). Treat
  nonblocking `WouldBlock` as a retryable lifecycle condition and add a
  red-capable event-loop regression seam. A transient user service with
  `Restart=on-failure` is being used only as a development-session workaround.
- Implemented in commits `f3bc791`, `5d5625d`, `29c8ea0`, `ca16f41`, and
  `8462754`.
- Added a logind lifecycle observer for the authoritative graphical session.
  It consumes session activity and lock state plus pre-sleep and pre-shutdown
  signals. Because a systemd user service belongs to `user@.service` rather
  than a session scope, it resolves the user's display session through
  logind's User object when `GetSessionByPID` is unavailable.
- Session Deactivation now removes the active Switching Session, rejects
  invocations and MRU observations while inactive, destroys the overlay and
  every capture allocation, and clears the cached Window order. Reactivation
  first crosses a compositor round-trip barrier, then rebuilds MRU Warm-up
  state from current observations; it never restores the old overlay.
- Output or layer loss after reveal cancels. Loss before visible reveal follows
  the atomic-readiness ADR by releasing partial resources and delegating the
  original direction to COSMIC's stock switcher. Any fatal compositor event
  loop error uses the same cleanup path before the service exits.
- Nonblocking Wayland reads now treat `WouldBlock` as retryable. Other read,
  dispatch, flush, or poll failures remain fatal after compositor-loss cleanup.
- The service-scenario cleanup matrix covers every interruption while preparing
  and after reveal for both DMA-BUF imported buffers and shared-memory buffers.
  It checks the input grab, overlay surface, capture sessions, outstanding
  frames, buffers, and last-frame references. A private title/pixel sentinel
  also verifies that invocation, effect, and diagnostic payloads exclude both.
  The future `doctor` command in ticket 13 consumes this same privacy-safe
  diagnostic source.
- No telemetry dependency, reporting endpoint, or automatic upload path was
  introduced.
- The required two-axis review finished with zero Standards findings and zero
  implementable Spec findings after review fixes.
- Final verification passed formatting, all-target type checking, strict
  all-target/all-feature Clippy, all 114 tests, an optimized release build, and
  an isolated real-process service startup smoke test.

## Answer

The Switcher Service now treats lock, suspend, user switching, output loss,
compositor loss, and session shutdown as explicit lifecycle boundaries. Every
path removes the input-grabbing overlay and Live Thumbnail resources without
activating a Window, while safe reactivation synchronizes current compositor
state before rebuilding MRU Warm-up. Transient `WouldBlock` reads no longer
terminate the resident service, and private Window titles and pixels remain
outside default external payloads.
