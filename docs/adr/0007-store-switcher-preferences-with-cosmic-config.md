# Store switcher preferences with cosmic-config

The product will persist Switcher Preferences exclusively through `cosmic-config`, using the stable application ID and an explicit versioned schema. Version 1 owns card size, background dimming, Hover Selection, thumbnail refresh limit, animations, and reveal delay.

Accessibility state and keyboard shortcuts remain authoritative COSMIC policies that the Switcher Service observes at runtime. Their values are never copied into the switcher's configuration, preventing stale or contradictory settings. Window Scope is the deliberate exception recorded in ADR-0012: All Workspaces does not copy COSMIC Workspace Policy, while Visible Workspaces consumes live compositor state only when the required membership snapshot is available.

Configuration loading validates every value and falls back to documented defaults for missing or invalid fields. Future releases migrate older schema versions deliberately rather than silently interpreting them as the newest representation.
