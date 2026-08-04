# COSMIC Window Switcher 0.2.1

This release recovers safely when the service temporarily loses its Window
tracking: the current shortcut delegates to COSMIC's stock switcher while the
service requests a fresh compositor snapshot for later requests.

This release supports only Pop!_OS 24.04 COSMIC Wayland sessions on `amd64`.
It does not claim support for GNOME, Ubuntu, Xorg, another desktop session, or
another architecture.

Installation is inert. Read `install-and-recovery.md` from the package or
repository, authenticate `SHA256SUMS` with the accompanying maintainer
signature, install the `.deb`, and run `cosmic-window-switcher enable` from a
COSMIC Wayland session.

The stock COSMIC switcher remains installed and directly callable. The package
contains no telemetry, crash uploader, automatic updater, apt repository,
sandboxed build, v2 controls, or persisted Window thumbnail data.
