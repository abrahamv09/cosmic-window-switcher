# 08 — Configure an accessible bilingual experience

**What to build:** A standalone native settings experience for the user-owned visual and performance choices, plus an overlay that follows COSMIC accessibility and locale policy. Saved changes become stable Session Preferences on the next invocation rather than reflowing an active grid.

**Blocked by:** 06 — Navigate the continuous grid with keyboard and mouse.

**Status:** resolved

- [x] Versioned Switcher Preferences persist card size, dimming, Refresh Ceiling, animations, and reveal delay through `cosmic-config`.
- [x] Defaults are medium cards, light dimming, 30 FPS ceiling, animations enabled, and 100 ms reveal delay.
- [x] Missing, invalid, and older configuration values recover or migrate without modifying COSMIC-owned settings.
- [x] Saved changes apply from the next Switching Session and never mutate an open grid.
- [x] Match-display refresh is clearly labeled as potentially expensive.
- [x] Screen readers receive meaningful card names, selected state, position, and interaction instructions when COSMIC accessibility is active.
- [x] High contrast and reduced motion override presentation as required by COSMIC policy.
- [x] All user-facing strings are complete in English and Spanish and follow the desktop locale without a separate language setting.
- [x] The settings view reports detected COSMIC shortcut state and directs users to COSMIC Keyboard Settings without implementing a competing recorder.

## Comments

- Implemented in commits `ea0435a`, `25cac65`, `c385acd`, `c2d11db`, and
  `0ed284b`.
- Added a version-1 typed `cosmic-config` schema with safe per-field fallback,
  recognized legacy aliases, canonical writes, documented defaults, and an
  immutable Session Preferences snapshot taken at each invocation.
- Added the standalone libcosmic settings window. It saves immediately, labels
  match-display refresh as the expensive choice, reports every effective
  forward/reverse COSMIC binding, and opens COSMIC Keyboard Settings rather
  than recording shortcuts.
- Added AccessKit listbox semantics, localized accessible names and interaction
  instructions for Hold and Latch Modes, COSMIC high-contrast rendering, and a
  real reveal fade that is bypassed by the animation preference or a standard
  desktop reduced-motion policy.
- English and Spanish now use Fluent resources for the overlay, settings,
  diagnostics, empty-title fallback, normal CLI help, and invalid-argument
  output. Locale negotiation skips empty or unsupported priority entries and
  honors ordered `LANGUAGE` lists.
- Runtime validation on 2026-07-30 found that the target Pop!_OS 24.04 COSMIC
  Settings portal returns `org.freedesktop.portal.Error.NotFound` for
  `org.freedesktop.appearance/reduced-motion`; `ReadAll` currently exposes only
  accent color, contrast, and color scheme. The client consumes the
  standardized reduced-motion key when the COSMIC portal publishes it. Until
  then, the app-owned Animations preference is the available motion control;
  no competing COSMIC accessibility value is persisted.
- Final verification passed formatting, strict Clippy, and all 87
  all-target/all-feature tests.
- The final two-axis review passed with zero Standards findings and zero
  implementable Spec findings.

## Answer

The switcher now has durable, session-stable visual and performance
preferences; a native settings surface that leaves shortcut ownership with
COSMIC; screen-reader, contrast, and motion-aware presentation; and complete
English/Spanish Fluent output following desktop locale priority. The target
COSMIC release does not yet publish its optional reduced-motion portal value,
which is recorded as an external capability absence rather than replaced with
an invented app-owned accessibility policy.
