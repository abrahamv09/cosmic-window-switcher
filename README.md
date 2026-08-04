# COSMIC Window Switcher

A fast, native window switcher for the COSMIC desktop, built in Rust.

COSMIC Window Switcher replaces the standard Alt+Tab experience with live
window previews and predictable most-recently-used (MRU) ordering. Tap the
shortcut for a quick switch without flashing an overlay, or hold it to browse a
responsive grid of open windows.

## Why?

This project is for people who want window switching to feel immediate and
consistent: the last window you used should be the first one offered, previews
should stay current, and selecting a window on another workspace should take
you there without moving it.

## Features

- Predictable MRU ordering maintained by a lightweight session service.
- Live thumbnails for native Wayland and compositor-managed XWayland windows.
- Quick switching without showing the overlay on a short key press.
- Keyboard and pointer navigation through a responsive card grid.
- One window list across all workspaces.
- Configurable card size, dimming, hover selection, refresh rate, animation,
  and reveal delay.
- High-contrast, reduced-motion, and AT-SPI accessibility support.
- English and Spanish interfaces.
- Reversible shortcut integration with automatic fallback to COSMIC's stock
  switcher when the custom overlay cannot start safely or Window tracking is
  temporarily unavailable; the service refreshes tracking for the next use.
- No telemetry and no window previews written to disk.

## Requirements

The current release supports:

- Pop!_OS 24.04
- COSMIC Wayland session
- `amd64` architecture

GNOME, Xorg, other desktop sessions, and other architectures are not currently
supported.

## Installation

Download the `.deb` and checksum files from the
[latest GitHub release](https://github.com/abrahamv09/cosmic-window-switcher/releases/latest).

Verify the downloaded files when using a signed release:

```sh
gpg --verify SHA256SUMS.asc SHA256SUMS
sha256sum --check SHA256SUMS
```

Install the package:

```sh
sudo apt install ./cosmic-window-switcher_0.2.1-1_amd64.deb
```

Installation does not alter your shortcuts or start the service. Enable the
integration explicitly from a COSMIC session:

```sh
cosmic-window-switcher enable
cosmic-window-switcher doctor
```

The switcher uses COSMIC's existing **Next Window** and **Previous Window**
shortcut actions. Their key bindings remain managed by **COSMIC Settings →
Input Devices → Keyboard → Shortcuts**. The recommended bindings are
`Alt+Tab` and `Alt+Shift+Tab`.

## Configuration

Open **COSMIC Window Switcher** from the application launcher, or run:

```sh
cosmic-window-switcher settings
```

Settings are saved immediately and apply to the next switching session. Reveal
delay options range from 20 to 200 ms; the default is 100 ms. **Select on
hover** is disabled by default, so moving the pointer does not interrupt
keyboard navigation; clicking a card still activates it.

Useful diagnostics:

```sh
cosmic-window-switcher status
cosmic-window-switcher doctor
systemctl --user status cosmic-window-switcher.service
```

## Disable or remove

Restore the stock COSMIC switcher before removing the package:

```sh
cosmic-window-switcher disable
sudo apt remove cosmic-window-switcher
```

Enablement and disablement are ownership-aware: shortcut commands are restored
only when they still contain values written by this application, so later
manual edits are preserved.

See [Install and recovery](docs/install-and-recovery.md) for upgrades,
troubleshooting, removal, and recovery from an interrupted operation.

## Build from source

The project requires Rust 1.95 or newer and the system dependencies listed in
[`debian/control`](debian/control).

```sh
git clone https://github.com/abrahamv09/cosmic-window-switcher.git
cd cosmic-window-switcher
cargo build --release
```

Run the development service from an active COSMIC Wayland session:

```sh
cargo run --release -- service
```

In another terminal, invoke or configure it with:

```sh
cargo run --release -- invoke next
cargo run --release -- settings
```

Run the project checks with:

```sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
```

## Current limitations

- Window scope is currently all workspaces; visible-workspace-only filtering is
  waiting on a complete initial workspace-membership snapshot from COSMIC.
- After the service restarts, MRU history enters a short warm-up period because
  focus order from before the restart cannot be reconstructed.
- Version 1 can switch and activate windows, but cannot close, minimize, or
  restore them from the grid.

See the [roadmap](ROADMAP.md) for planned work and the
[architecture decisions](docs/adr/) for design rationale.

## License

Licensed under [GPL-3.0-only](LICENSE).
