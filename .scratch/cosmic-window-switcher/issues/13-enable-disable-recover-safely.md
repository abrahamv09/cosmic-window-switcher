# 13 — Enable, disable, and recover safely

**What to build:** A reversible integration lifecycle that lets the user explicitly make COSMIC Window Switcher handle existing semantic switching actions, inspect its health, return to the stock switcher, and preserve any shortcut edits they make later.

**Blocked by:** 04 — Navigate an icon-and-title overlay; 08 — Configure an accessible bilingual experience.

**Status:** resolved

- [x] Package installation alone changes no shortcut or starts no enabled app-owned integration without user consent.
- [x] Explicit enablement starts the resident service with the COSMIC Session and transactionally overrides only `WindowSwitcher` and `WindowSwitcherPrevious` user semantic commands.
- [x] Existing key-to-semantic-action bindings remain owned and editable through COSMIC Settings.
- [x] Enablement records exact prior semantic-command values and rolls back fully after an interrupted write.
- [x] Disable or uninstall restores/removes a value only when its current value still matches the app-owned command.
- [x] Repeated enable and disable operations are idempotent and preserve manual edits made after enablement.
- [x] Stock fallback invokes the launcher directly in the requested direction and cannot recurse through the overridden semantic action.
- [x] `status` and `doctor` report service, capability, Capture Backend, MRU Warm-up, and shortcut-ownership state without private Window data.
- [x] GNOME, Ubuntu, Xorg, and other non-COSMIC sessions are rejected without starting the service or changing shortcuts.

## Comments

- Added explicit `enable` and `disable` commands backed by an atomic lifecycle
  journal. Fresh enablement records the exact prior user semantic commands,
  updates both actions in one `cosmic-config` value, and rolls back shortcut and
  service state if enablement cannot finish.
- Disablement restores or removes each semantic command only while its current
  value still equals the corresponding app-owned invocation. Repeated lifecycle
  operations preserve later manual edits, and user key-to-action binding files
  remain untouched.
- Added inert systemd user-unit and D-Bus activation definitions for the package
  installed by ticket 14. Explicit enablement links the unit into the COSMIC
  session target and starts it immediately; installation alone does neither.
- `status` and `doctor` combine a non-activating D-Bus ownership check with the
  existing privacy-safe service diagnostics and current shortcut ownership.
- Invocation and lifecycle mutation now reject non-COSMIC and non-Wayland
  sessions before D-Bus activation, stock fallback, service control, or shortcut
  writes.
- The lifecycle sandbox covers existing and absent prior values, untouched key
  bindings, interrupted enablement rollback, later manual edits, repeated
  enable/disable, absent-service diagnostics, direct fallback isolation, and
  unsupported-session rejection.
- Verification passed formatting, all-target type checking, strict
  all-target/all-feature Clippy, all 116 tests, and an optimized release build.

## Answer

COSMIC Window Switcher now has an explicit, reversible integration lifecycle.
It owns only the two semantic switching commands while their values match its
installed invocations, leaves COSMIC Settings in control of key assignments,
and returns safely to stock behavior without overwriting subsequent user edits.
