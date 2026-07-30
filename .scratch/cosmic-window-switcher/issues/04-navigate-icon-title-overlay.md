# 04 — Navigate an icon-and-title overlay

**What to build:** A visible native Switcher Grid using icon-and-title cards, with the complete keyboard Switching Session lifecycle. Users can hold a modifier and cycle, use a modifier-free shortcut in Latch Mode, reverse direction, wrap, commit, or cancel without unstable ordering or focus surprises.

**Blocked by:** 03 — Quick-switch to the previous Window.

**Status:** resolved

- [x] The overlay appears only after Session Readiness and only on the Session Display.
- [x] Forward invocation initially selects the previous Window; reverse invocation initially selects the last MRU item.
- [x] Repeated forward and reverse navigation wrap without changing MRU Order.
- [x] Hold Mode commits when the last initial Alt, Ctrl, or Super modifier is released; releasing Shift alone never commits.
- [x] Latch Mode remains open until Enter activates or Escape cancels.
- [x] Escape restores the Window focused when the Switching Session began.
- [x] The Session Window Set is stable: new Windows wait, closed Windows disappear, and selection advances deterministically when its Window closes.
- [x] Every card displays an application icon, Window title, and accessible selected state.
- [x] The overlay remains above fullscreen content without changing its fullscreen state.

## Comments

- Implemented in commits `9c9336d`, `fac99b1`, and `4258a66`.
- Added the complete forward/reverse Switching Session lifecycle with stable wrapping, Hold and Latch Modes, deterministic closure handling, cancellation without focus activation, and an invocation-time Session Window Set.
- Added a Session Display-targeted overlay-layer Switcher Grid with installed SVG/raster application icons, monogram fallbacks, Window titles, selected styling, output-constrained minimal scrolling, and shared-memory rendering above fullscreen content.
- Added production AccessKit/AT-SPI listbox semantics. The accessibility tree is published only when the ready overlay is revealed and is hidden with the visual overlay.
- Focused tests cover Switching Session, Switcher Service, and public Switcher Grid seams. Final verification passed formatting, strict Clippy, all 41 all-target/all-feature tests, and a release build.
- A live COSMIC service/status smoke test passed. Interactive overlay invocation remains a human visual smoke check because it takes exclusive keyboard focus.
- Two-axis review initially found domain-vocabulary, icon, accessibility, snapshot, viewport, and lifecycle gaps. All findings were corrected; the final Standards and Spec passes were clean.
