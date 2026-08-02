# 09 — Stay responsive with GPU-native capture

**What to build:** An automatic high-performance capture path that prefers DMA-BUF when the entire compositor-to-renderer contract is compatible and falls back to proven SHM behavior otherwise. Under load, input and the selected preview remain responsive while every other visible item receives fair updates.

**Blocked by:** 05 — Show Live Thumbnails through shared memory; 08 — Configure an accessible bilingual experience.

**Status:** ready-for-human

- [x] DMA-BUF is selected only when device, formats, modifiers, allocation, synchronization, import, and release behavior are mutually compatible.
- [x] Any incompatible or failed DMA-BUF negotiation falls back to SHM automatically without requiring a user preference.
- [x] Diagnostics report the active Capture Backend and fallback reason without Window titles or pixels.
- [x] Scheduling prioritizes keyboard and pointer input, then the selected thumbnail, then fair round-robin work for other visible items.
- [x] The user-selected Refresh Ceiling is treated as a maximum for changed content, not a guaranteed duplicate-frame rate.
- [x] The idle service performs no capture and has effectively zero sustained capture CPU use.
- [ ] On the development baseline, selection responds within one display frame and the visible overlay is ready within 50 ms after its configured delay.
- [ ] Ten visible Windows remain smooth at the default ceiling without input stalls, and overload recovers automatically.

## Comments

- Implemented in commits `32331ab` and `4ee1e64`, with the final event-loop
  priority and review cleanup in the ticket handoff commit.
- Added a seven-stage DMA-BUF compatibility contract. The production software
  overlay currently has no DMA-BUF import boundary, so negotiation truthfully
  selects SHM with `import-unavailable`; it cannot claim an active GPU path
  until a renderer can satisfy the complete contract.
- Added backward-compatible, privacy-safe D-Bus diagnostics for the active
  Capture Backend and fallback reason, including English and Spanish output
  and an explicit pre-negotiation state.
- Capture work now runs only after queued keyboard, pointer, and invocation
  input. Each opportunity requests the selected thumbnail first and at most
  one fair round-robin background thumbnail, maintains one outstanding request
  per stream, coalesces completed thumbnail paints, and leaves the poll timeout
  unset when no capture stream is active.
- An optimized service run in an isolated user-session bus remained idle at
  0.0% CPU with 13,472 KiB RSS. The deterministic ten-window scheduler test
  proves bounded, fair overload recovery without duplicate requests.
- The two-axis review passed with zero remaining Standards findings and no
  remaining implementable Spec findings. Physical-baseline validation of the
  final two criteria still requires a human session with ten visible Windows,
  so the ticket is handed off as `ready-for-human` rather than marked resolved.
- Final verification passed formatting, all-target type checking, strict
  all-target/all-feature Clippy, all 107 tests, and an optimized release build.
- Manual restart exposed a latent runtime-feature conflict from the settings
  implementation: libcosmic enabled zbus's Tokio backend while the resident
  service uses zbus's blocking API. Switching libcosmic to its supported smol
  executor kept the UI asynchronous and restored zbus's compatible async-io
  backend. An isolated real-process D-Bus/Wayland startup reproduced the exact
  reactor panic before the change and remained alive without a panic after it.
- Human validation with nine open Windows reported smooth Live Thumbnails at
  30 FPS, 60 FPS, and match-display refresh. It also exposed that Large cards
  used two exact rows with no continuation cue and devoted too much space to
  metadata. Overflow layouts now include a clipped, live half-row when the
  remaining viewport can hold it, and cards use a smaller footer, icon, title,
  and thumbnail inset so Window content remains dominant.
- CLI invocation deliberately enters Latch Mode; pressing the still-stock
  COSMIC Alt+Tab action during that temporary test launches the stock switcher
  on top. The semantic shortcut remains COSMIC-owned until ticket 13 installs
  the reversible app command. Plain Tab and Shift+Tab navigate Latch Mode.
- The tester also reported thumbnails turning gray around COSMIC screenshot
  capture. The supplied screenshots contain live content, so that state still
  needs a post-capture screenshot or exact persistence steps before it can be
  reproduced and diagnosed without guessing.
- Follow-up visual validation requested count-responsive cards. One through
  five Windows now occupy one row, six use two rows of three, and larger sets
  use rows of at most five. Width and height adapt together so two complete
  rows remain visible and a third row can peek by half. Medium and Small
  preserve their preference meaning as 90% and 80% density multipliers.
  Metadata dimensions remain capped as previews grow, and the title line is
  vertically centered.
- Live compositor inventory identified icon aliases `vlc` and
  `MongoDB Compass`, while their installed Flatpak desktop/icon IDs are
  `org.videolan.VLC` and `com.mongodb.Compass`. The icon resolver now maps raw
  compositor identities through standard desktop-entry ID, name, and
  `StartupWMClass` metadata instead of showing fallback monograms.
- Follow-up keyboard validation added arrow-key navigation using the live grid
  column count. Left and Right follow continuous row order, so one press crosses
  between the end of one row and the start of the next; Up and Down stay within
  their visual column. Held arrow keys repeat, while Tab and Shift+Tab retain
  their wrapping MRU behavior.
- Overflow discovery is symmetric: when selection scrolls toward later rows,
  the preceding row peeks into the top of the Grid Viewport by half, just as a
  following row peeks into the bottom when selection is near the beginning.
