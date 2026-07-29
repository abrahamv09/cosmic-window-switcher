# 13 — Enable, disable, and recover safely

**What to build:** A reversible integration lifecycle that lets the user explicitly make COSMIC Window Switcher handle existing semantic switching actions, inspect its health, return to the stock switcher, and preserve any shortcut edits they make later.

**Blocked by:** 04 — Navigate an icon-and-title overlay; 08 — Configure an accessible bilingual experience.

**Status:** ready-for-agent

- [ ] Package installation alone changes no shortcut or starts no enabled app-owned integration without user consent.
- [ ] Explicit enablement starts the resident service with the COSMIC Session and transactionally overrides only `WindowSwitcher` and `WindowSwitcherPrevious` user semantic commands.
- [ ] Existing key-to-semantic-action bindings remain owned and editable through COSMIC Settings.
- [ ] Enablement records exact prior semantic-command values and rolls back fully after an interrupted write.
- [ ] Disable or uninstall restores/removes a value only when its current value still matches the app-owned command.
- [ ] Repeated enable and disable operations are idempotent and preserve manual edits made after enablement.
- [ ] Stock fallback invokes the launcher directly in the requested direction and cannot recurse through the overridden semantic action.
- [ ] `status` and `doctor` report service, capability, Capture Backend, MRU Warm-up, and shortcut-ownership state without private Window data.
- [ ] GNOME, Ubuntu, Xorg, and other non-COSMIC sessions are rejected without starting the service or changing shortcuts.

