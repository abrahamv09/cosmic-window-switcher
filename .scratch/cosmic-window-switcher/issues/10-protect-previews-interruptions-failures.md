# 10 — Protect previews across interruptions and failures

**What to build:** Make privacy and focus safety hold when the desktop changes unexpectedly. Lock, suspend, user switch, output loss, compositor loss, Window closure, and session shutdown cancel cleanly, release every preview resource, and never resurrect a stale overlay after resume.

**Blocked by:** 07 — Respect COSMIC Window, workspace, and display state; 09 — Stay responsive with GPU-native capture.

**Status:** ready-for-agent

- [ ] Screen lock, suspend, user switch, and COSMIC session shutdown cancel without activating another Window.
- [ ] Session Deactivation destroys overlay surfaces, capture sessions, imported buffers, and last-frame references.
- [ ] MRU observation pauses outside an active unlocked COSMIC Session and rebuilds safely after resume.
- [ ] Output removal or compositor disconnect during switching cannot leave an input-grabbing invisible surface.
- [ ] A selected Window closing advances selection when possible and cancels safely when no candidates survive.
- [ ] Default journal output, D-Bus traffic, crash diagnostics, and `doctor` output contain no Window pixels or titles.
- [ ] No telemetry or automatic crash upload is introduced.
- [ ] Lifecycle and service-scenario tests verify cleanup after every interruption point and backend state.

