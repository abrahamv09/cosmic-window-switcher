# 06 — Navigate the continuous grid with keyboard and mouse

**What to build:** A Windows-style continuous Switcher Grid that remains readable with many Windows. Fixed-size cards wrap into rows without pagination, navigation reveals the selected row, capture follows the Grid Viewport, and pointer users can select, activate, and cancel safely.

**Blocked by:** 05 — Show Live Thumbnails through shared memory.

**Status:** resolved

- [x] All Switcher Items occupy one continuous MRU layout with no page boundaries or page indicators.
- [x] Cards retain the selected size preset instead of shrinking merely to fit more Windows.
- [x] Keyboard navigation scrolls only enough to reveal the selected row and never changes MRU traversal order.
- [x] Only rows intersecting the Grid Viewport continue Live Thumbnail capture; off-screen rows resume when revealed.
- [x] Pointer hover is ignored until the pointer actually moves after reveal.
- [x] Hover selects, clicking a card activates it, and clicking outside cancels without changing original focus.
- [x] Layout remains usable across large Window counts, long titles, varied aspect ratios, and supported fractional scales.
- [x] Geometry and service-scenario tests assert visible behavior rather than renderer internals.

## Comments

- Implemented in commits `f5055e5`, `378af72`, and `066e671`.
- Added a continuous fixed-card Switcher Grid with small, medium, and large presets, selected-row reveal, centered full-display presentation, and exact fractional-scale buffer geometry through `wp_fractional_scale_v1` plus `wp_viewporter`.
- Added safe pointer interaction: reveal-time pointer entry is inert, post-reveal motion selects, a primary click commits only when press and release target the same card, and a completed background click cancels without Window activation.
- Grid Viewport changes now drive Live Thumbnail stream creation and release through the same visible-Window geometry used by the presentation path; off-screen rows resume capture when navigation reveals them.
- Geometry and fake-adapter service scenarios cover fixed presets, large Window counts, long titles, fractional scale, minimal row reveal, capture suspension/resumption, hover, activation, withdrawn clicks, and cancellation.
- Final verification passed formatting, strict Clippy, and all 61 all-target/all-feature tests.
- The final two-axis review passed cleanly with zero Standards findings and zero Spec findings.
