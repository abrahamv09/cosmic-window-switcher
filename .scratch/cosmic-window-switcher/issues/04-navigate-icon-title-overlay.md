# 04 — Navigate an icon-and-title overlay

**What to build:** A visible native Switcher Grid using icon-and-title cards, with the complete keyboard Switching Session lifecycle. Users can hold a modifier and cycle, use a modifier-free shortcut in Latch Mode, reverse direction, wrap, commit, or cancel without unstable ordering or focus surprises.

**Blocked by:** 03 — Quick-switch to the previous Window.

**Status:** ready-for-agent

- [ ] The overlay appears only after Session Readiness and only on the Session Display.
- [ ] Forward invocation initially selects the previous Window; reverse invocation initially selects the last MRU item.
- [ ] Repeated forward and reverse navigation wrap without changing MRU Order.
- [ ] Hold Mode commits when the last initial Alt, Ctrl, or Super modifier is released; releasing Shift alone never commits.
- [ ] Latch Mode remains open until Enter activates or Escape cancels.
- [ ] Escape restores the Window focused when the Switching Session began.
- [ ] The Session Window Set is stable: new Windows wait, closed Windows disappear, and selection advances deterministically when its Window closes.
- [ ] Every card displays an application icon, Window title, and accessible selected state.
- [ ] The overlay remains above fullscreen content without changing its fullscreen state.

