# 08 — Configure an accessible bilingual experience

**What to build:** A standalone native settings experience for the user-owned visual and performance choices, plus an overlay that follows COSMIC accessibility and locale policy. Saved changes become stable Session Preferences on the next invocation rather than reflowing an active grid.

**Blocked by:** 06 — Navigate the continuous grid with keyboard and mouse.

**Status:** ready-for-agent

- [ ] Versioned Switcher Preferences persist card size, dimming, Refresh Ceiling, animations, and reveal delay through `cosmic-config`.
- [ ] Defaults are medium cards, light dimming, 30 FPS ceiling, animations enabled, and 100 ms reveal delay.
- [ ] Missing, invalid, and older configuration values recover or migrate without modifying COSMIC-owned settings.
- [ ] Saved changes apply from the next Switching Session and never mutate an open grid.
- [ ] Match-display refresh is clearly labeled as potentially expensive.
- [ ] Screen readers receive meaningful card names, selected state, position, and interaction instructions when COSMIC accessibility is active.
- [ ] High contrast and reduced motion override presentation as required by COSMIC policy.
- [ ] All user-facing strings are complete in English and Spanish and follow the desktop locale without a separate language setting.
- [ ] The settings view reports detected COSMIC shortcut state and directs users to COSMIC Keyboard Settings without implementing a competing recorder.

