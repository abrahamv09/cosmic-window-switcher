# 05 — Show Live Thumbnails through shared memory

**What to build:** Replace placeholder content with damage-driven Live Thumbnails using the required shared-memory Capture Backend. Users see current uncropped Window contents, while a failure isolated to one Window leaves a useful icon-and-title card instead of breaking the Switching Session.

**Blocked by:** 04 — Navigate an icon-and-title overlay.

**Status:** ready-for-agent

- [ ] Each visible card can create a capture source for its Window and negotiate exact source dimensions and supported SHM formats.
- [ ] Captured content fits within the card without cropping, distortion, or loss of the icon and title.
- [ ] Each capture session keeps no more than one frame outstanding and refreshes on compositor damage up to the Refresh Ceiling.
- [ ] Frame failure, stopped capture, protected content, or unsupported XWayland capture degrades only the affected Switcher Item.
- [ ] Closing a Window or ending the Switching Session releases its capture session and every associated buffer.
- [ ] No normal, error, test, or diagnostic path writes thumbnail pixels to disk.
- [ ] Live contract tests cover changed and unchanged content, minimized Windows, Native Wayland Windows, representative XWayland Windows, failure, and session stop.

