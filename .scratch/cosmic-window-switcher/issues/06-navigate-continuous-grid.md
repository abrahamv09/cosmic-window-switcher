# 06 — Navigate the continuous grid with keyboard and mouse

**What to build:** A Windows-style continuous Switcher Grid that remains readable with many Windows. Fixed-size cards wrap into rows without pagination, navigation reveals the selected row, capture follows the Grid Viewport, and pointer users can select, activate, and cancel safely.

**Blocked by:** 05 — Show Live Thumbnails through shared memory.

**Status:** ready-for-agent

- [ ] All Switcher Items occupy one continuous MRU layout with no page boundaries or page indicators.
- [ ] Cards retain the selected size preset instead of shrinking merely to fit more Windows.
- [ ] Keyboard navigation scrolls only enough to reveal the selected row and never changes MRU traversal order.
- [ ] Only rows intersecting the Grid Viewport continue Live Thumbnail capture; off-screen rows resume when revealed.
- [ ] Pointer hover is ignored until the pointer actually moves after reveal.
- [ ] Hover selects, clicking a card activates it, and clicking outside cancels without changing original focus.
- [ ] Layout remains usable across large Window counts, long titles, varied aspect ratios, and supported fractional scales.
- [ ] Geometry and service-scenario tests assert visible behavior rather than renderer internals.

