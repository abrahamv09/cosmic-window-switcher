# Ship as a standalone native shell client

The first release will be a standalone Rust and libcosmic Wayland client that COSMIC launches for its window-switcher shortcut. This provides the required shell-level overlay, window control, workspace awareness, and live capture without maintaining forks of `cosmic-comp`, `cosmic-launcher`, or `cosmic-settings`; COSMIC currently has no suitable third-party shell-extension or settings-page mechanism.

The client supports both Native Wayland Windows and XWayland Windows exposed by the COSMIC compositor. It is intentionally COSMIC-specific: in GNOME, Ubuntu, or any other non-COSMIC session it exits cleanly without activating its service or changing shortcuts.

## Considered Options

- A COSMIC panel applet has the wrong surface and lifecycle for a transient desktop-wide switcher.
- A compositor, launcher, or settings fork would integrate deeply but impose ongoing synchronization and packaging costs.
- The standalone boundary preserves a path to contribute the switcher upstream later.
