# Install and recover COSMIC Window Switcher

Version 1 supports only Pop!_OS 24.04 COSMIC Wayland sessions on `amd64`.
GNOME, Ubuntu, Xorg, other desktops, and other architectures are unsupported;
the commands refuse to change integration settings in those sessions.

## Authenticate and install

Download the `.deb`, `SHA256SUMS`, and `SHA256SUMS.asc` files from the same
GitHub Release. Import the maintainer's public signing key through a separately
trusted channel, then authenticate the manifest and package:

```sh
gpg --verify SHA256SUMS.asc SHA256SUMS
sha256sum --check SHA256SUMS
sudo apt install ./cosmic-window-switcher_0.1.0-1_amd64.deb
```

Installation is inert. It installs the executable, settings launcher, icon,
defaults, documentation, D-Bus activation file, and user service, but it does
not start the service or change a shortcut.

From a COSMIC Wayland session, explicitly enable the integration:

```sh
cosmic-window-switcher enable
cosmic-window-switcher status
```

Enablement changes only the commands behind COSMIC's existing
`WindowSwitcher` and `WindowSwitcherPrevious` semantic actions. Open
**COSMIC Settings → Input Devices → Keyboard → Shortcuts** to assign the
forward and reverse Window-switching actions to the desired keys. The
recommended mappings are `Alt+Tab` and `Alt+Shift+Tab`; COSMIC remains the
owner of those key-to-action mappings.

## Diagnose and recover

Use both diagnostics when the custom Switcher Grid does not open:

```sh
cosmic-window-switcher status
cosmic-window-switcher doctor
systemctl --user status cosmic-window-switcher.service
```

`status` and `doctor` report the COSMIC Session, service, negotiated
capabilities, Capture Backend, MRU Warm-up, Window Scope, and shortcut
ownership. A missing required capability prevents a partial grid and delegates
the same direction to `/usr/bin/cosmic-launcher`, COSMIC's stock switcher.

The stock commands remain directly callable at all times:

```sh
cosmic-launcher alt-tab
cosmic-launcher shift-alt-tab
```

If a capability or service recovery attempt fails, run `disable` to stop the
service and restore only semantic command values still owned by this package:

```sh
cosmic-window-switcher disable
```

Later manual command edits are preserved. If a removal was interrupted, reinstall
the same package, run `cosmic-window-switcher disable`, and retry removal.

## Upgrade, remove, and purge

Upgrade in place without disabling; the package does not rewrite per-user
integration state:

```sh
sudo apt install ./cosmic-window-switcher_NEW-VERSION_amd64.deb
cosmic-window-switcher doctor
```

For the clearest recovery path, explicitly disable before removal:

```sh
cosmic-window-switcher disable
sudo apt remove cosmic-window-switcher
```

During removal, the package also performs an ownership-safe fallback cleanup
for enabled users whose lifecycle journal is in the standard XDG state
location. It restores or removes only commands that still exactly equal the
app-owned values. Purge removes package-managed files and has the same shortcut
safety behavior; app preference files in a user's home remain user data.

```sh
sudo apt purge cosmic-window-switcher
```
