# 05 — Show Live Thumbnails through shared memory

**What to build:** Replace placeholder content with damage-driven Live Thumbnails using the required shared-memory Capture Backend. Users see current uncropped Window contents, while a failure isolated to one Window leaves a useful icon-and-title card instead of breaking the Switching Session.

**Blocked by:** 04 — Navigate an icon-and-title overlay.

**Status:** resolved

- [x] Each visible card can create a capture source for its Window and negotiate exact source dimensions and supported SHM formats.
- [x] Captured content fits within the card without cropping, distortion, or loss of the icon and title.
- [x] Each capture session keeps no more than one frame outstanding and refreshes on compositor damage up to the Refresh Ceiling.
- [x] Frame failure, stopped capture, protected content, or unsupported XWayland capture degrades only the affected Switcher Item.
- [x] Closing a Window or ending the Switching Session releases its capture session and every associated buffer.
- [x] No normal, error, test, or diagnostic path writes thumbnail pixels to disk.
- [x] Live contract tests cover changed and unchanged content, minimized Windows, Native Wayland Windows, representative XWayland Windows, failure, and session stop.

## Comments

- Implemented in commits `97c1d94`, `c798975`, `fd90852`, `d682bf3`, and `7599afc`.
- Added damage-driven, exact-size shared-memory capture for every visible Window, with a 30 FPS Refresh Ceiling, one outstanding frame per stream, buffer reuse, all compositor transforms, and aspect-preserving uncropped rendering that retains each card's icon and title.
- Capture setup, frame, stopped-session, protected-content, and unsupported-format failures are isolated to the affected Switcher Item and immediately restore its icon-and-title fallback.
- The shared raw-protocol capture adapter explicitly destroys every frame before its session and releases every SHM buffer on completion, failure, Window removal, session stop, and backend drop. Neither the service nor its diagnostic probe writes thumbnail pixels to disk.
- Focused contract tests cover negotiation, exact allocation, all eight transforms, fitting, damage scheduling and unchanged suppression, isolated failure, fallback rendering, and complete stream release.
- Final verification passed formatting, strict Clippy, all 52 all-target/all-feature tests, and a release build.
- Live COSMIC contract probes captured changing and unchanged Native Wayland content plus a representative XWayland `Xmessage` Window in both visible and minimized states. Shutdown reported zero outstanding frame proxies and released all sessions and SHM allocations.
- The final two-axis review passed cleanly with zero Standards findings and zero Spec findings.
