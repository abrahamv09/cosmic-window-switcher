# COSMIC Window Switcher

COSMIC Window Switcher is a native Rust Window switcher for the COSMIC desktop.
The executable includes a resident Switcher Service plus an integration probe
for Window switching. The service
observes Window focus metadata throughout a COSMIC Session and keeps the current
MRU Order without opening a visible Window or starting thumbnail capture. Its
quick-switch path captures the initial hold modifiers, selects in the requested
MRU direction, and activates on release without flashing an overlay. Longer
holds reveal a native icon-and-title Switcher Grid on the best available
Session Display.

The stable application identity is
`io.github.abrahamv09.CosmicWindowSwitcher`. The project is licensed under
GPL-3.0-only.

## Build and test

```sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
cargo build --release
```

## Run the two-Window probe

Run this from a Pop!_OS 24.04 COSMIC Wayland session with at least two open
Windows:

```sh
cargo run --release -- probe
```

The probe prints every compositor-managed Window's opaque identity and
application identity. Add `--include-titles` for the temporary mixed-client
verification when title output is needed:

```sh
cargo run --release -- probe --include-titles
```

Title logging is opt-in because titles can contain private content. The probe
starts one-frame capture for each discovered Window, but retains no pixels after
the process exits and never writes Window pixels to disk. A successful capture
reports its exact compositor-provided dimensions, allocated byte count, and SHM
format.

For the ticket-5 Live Thumbnail contract, repeat capture only after compositor
damage and cap the probe at three frames per Window:

```sh
cargo run --release -- probe --live-thumbnails
```

Leave one Window unchanged, change content in another, exercise minimization or
a protected/unsupported Window where available, then press Escape. The report
distinguishes damage-driven updates from a Window that emitted no duplicate
frame and confirms that session-stop cleanup released every remaining capture
session and SHM allocation.

The transparent overlay requests exclusive keyboard focus:

- Tab cycles the selected Window.
- Escape cancels without issuing an activation request.
- Releasing the Alt, Ctrl, or Super modifier that was held when the overlay
  received focus activates the selected Window once.

For the mixed-client check, open one Native Wayland Window and one XWayland
Window before running the probe. Each remains in the discovery report even if
capture fails; a capture failure is printed against that Window identity.

The probe intentionally has no visible grid yet. It is a ticket 1 tracer bullet,
not the finished switcher service.

## Run the Switcher Service

Package installation is inert: it neither starts the resident service nor
changes COSMIC shortcuts. After installation, explicitly enable the integration
from a COSMIC Wayland session:

```sh
cosmic-window-switcher enable
```

Enablement starts `cosmic-window-switcher.service` with the COSMIC user session
and changes only the user-level commands for the existing `WindowSwitcher` and
`WindowSwitcherPrevious` semantic actions. COSMIC Settings continues to own the
key-to-action bindings. The prior command values are recorded before the atomic
shortcut update.

For development without the installed user unit, start the resident service
directly from a COSMIC Wayland session:

```sh
cargo run --release -- service
```

Only one process can own the service's user-session D-Bus name. Inspect service,
capability, Capture Backend, MRU Warm-up, and shortcut-ownership health with:

```sh
cosmic-window-switcher status
cosmic-window-switcher doctor
```

When the service is running, both commands report either `mru_history: accurate`
or `mru_history: warm-up`. MRU Warm-up means the service restarted with
pre-existing Windows whose relative focus history cannot be reconstructed.
Opaque Window identities are shown so the MRU Order can be verified; Window
titles and pixels are never included.

Return to the stock switcher before removing the package with:

```sh
cosmic-window-switcher disable
```

Disablement stops the resident service and restores or removes a semantic
command only while its current value still matches the app-owned invocation.
Manual command edits made after enablement are left intact. Repeating either
lifecycle command is safe. GNOME, Ubuntu, Xorg, and other non-COSMIC sessions
are rejected before the service or shortcut configuration is touched.
The package-removal workflow can call the internal
`disable --uninstall` cleanup path without a graphical-session environment; it
applies the same ownership checks and treats service shutdown as best effort so
shortcut restoration is not skipped during removal.

## Configure the switcher

Open the standalone native libcosmic settings window:

```sh
cargo run --release -- settings
```

The window follows the desktop locale in English or Spanish and saves each
change immediately through the version-1 `cosmic-config` namespace
`io.github.abrahamv09.CosmicWindowSwitcher`. It owns only card size, background
dimming, Refresh Ceiling, animations, and reveal delay. Defaults are medium
cards, light dimming, 30 FPS, animations enabled, and a 100 ms reveal delay.
Missing or invalid fields recover independently, and recognized legacy values
migrate without changing COSMIC-owned settings.

Saved changes apply when the next Switching Session starts; an open Switcher
Grid keeps its immutable Session Preferences. Match-display refresh is labeled
as the higher-resource choice and follows the current mode of the Session
Display. The settings window reports COSMIC's current forward and reverse
shortcut assignments and opens COSMIC Keyboard Settings for changes rather
than recording shortcuts itself.

## Invoke quick switching

With the resident service running, request forward or reverse switching through
the versioned user-session D-Bus interface:

```sh
cargo run --release -- invoke next
cargo run --release -- invoke previous
```

Forward initially selects the second Window in MRU Order; reverse initially
selects the final Window. Releasing the last initially held Alt, Ctrl, or Super
modifier activates the selection. An invocation without a hold modifier uses
Latch Mode, where Enter activates and Escape cancels. A single Eligible Window
is a no-op.

If the reveal delay expires, the service reveals a centered Switcher Grid in
stable MRU Order. Each visible Switcher Item starts one damage-driven
shared-memory capture stream. The compositor's exact source dimensions and a
supported four-byte SHM format are negotiated before the first frame; full
Window contents are then fitted into the card without cropping or distortion.
The default Refresh Ceiling is 30 FPS, unchanged Windows do not produce
duplicate frames, and a stream never has more than one request outstanding.
Rows outside the Grid Viewport release their capture streams.

The grid adapts card geometry to the Window count and available display height.
One through five Windows occupy one row, six use two rows of three, and larger
sets use rows of at most five. When more than two rows exist, two complete rows
and half of the adjacent overflow row remain visible. The selected Card Size
preset scales this geometry, and fractional-scale surface hints produce the
correct buffer dimensions. Pointer entry after reveal is inert until the
pointer moves; motion over a card selects it. A primary-button click activates
only when press and release complete on the same card, while a completed click
on the dimmed background cancels without activating a Window.

Every card retains its installed application icon (or an
application-identity monogram), Window title, and high-contrast selected state.
A denied, stopped, failed, protected, or unsupported Window capture degrades
only that Switcher Item to its icon-and-title card. Native Wayland Windows and
compositor-managed XWayland Windows use the same capture path. The same names,
positions, focus, and selected state are exposed to assistive technology
through AT-SPI. `Tab` moves forward, `Shift+Tab` moves backward, and both
directions wrap. Left and Right follow the continuous row order, including
crossing between rows; Up and Down retain the current visual column. The grid
remains above fullscreen content without changing the selected Window's
fullscreen state. Closed Windows disappear without
reordering survivors, while Windows opened during switching wait for the next
Switching Session. When all rows do not fit, rendering follows the selected row
so it remains visible.

The overlay follows COSMIC high-contrast state and suppresses animation when
the standardized desktop Settings portal reports a reduced-motion preference.
The current Pop!_OS 24.04 COSMIC portal does not yet publish that optional
value, so the app-owned Animations toggle is the available motion control on
that target. Its AT-SPI tree exposes localized English or Spanish names,
selected state, set position and size, focus, and keyboard interaction
instructions whenever assistive technology activates the tree.

The default Window Scope is All Workspaces. Every independently exposed
application Window enters one global MRU Order regardless of its workspace.
Minimized Windows, dialogs and utilities, Native Wayland Windows, and
compositor-managed XWayland Windows remain eligible. Layer-shell panels, docks,
menus, notifications, and overlays never enter the foreign-toplevel Window
registry.

This scope is deliberate, not a guess from incomplete state. The target
`cosmic-comp` advertises per-Window workspace membership but does not emit its
initial membership snapshot, so Visible Workspaces cannot be derived
authoritatively. Delegating every invocation would make the custom switcher
unavailable, while patching the compositor would delay use and create an
external maintenance dependency. [ADR-0012](docs/adr/0012-default-to-all-workspaces.md)
records the decision and alternatives.

The visible-workspace derivation remains available in the domain model for a
future stricter mode. It derives the Visible Workspace Set from live
`ext-workspace` groups, their assigned outputs, active and hidden state, and
each Window's committed COSMIC workspace membership. A spanning group includes
its active workspace across every display; separate-display groups contribute
the active workspace from each display.

Selecting a Window on another workspace uses COSMIC's normal activation
behavior to follow that Window to its existing workspace without relocating
it. This app does not show a workspace view or expose workspace targets;
COSMIC's workspace interface owns workspace organization. [ADR-0013](docs/adr/0013-leave-workspace-management-to-cosmic.md)
records that product boundary.

The service creates exactly one overlay. It prefers the output containing the
Window that was focused at invocation; when COSMIC omits per-Window output
membership, All Workspaces deterministically uses an output assigned to a
workspace group. Selecting a minimized Window uses COSMIC's normal activation
request, which restores and focuses it. Activation does not issue any
fullscreen-state request, so a fullscreen Window remains fullscreen.

`status` reports `window_scope: all-workspaces` and
`workspace_filtering: not-required`. It continues to diagnose missing COSMIC
workspace snapshots, but those snapshots do not block All Workspaces
invocations. The packaged compositor limitation recorded in
`.scratch/cosmic-window-switcher/upstream/` therefore does not force the custom
switcher to delegate.

If the grid cannot be rendered or targeted to the Session Display before
Session Readiness times out, the resident service delegates that invocation to
the stock switcher instead of leaving an invisible session open.

Live Thumbnail pixels stay only in compositor, process, and anonymous
shared-memory allocations. Closing a Window, leaving the Grid Viewport, ending
the Switching Session, or terminating the service destroys the corresponding
capture sessions and releases their buffers. No capture or diagnostic path
writes those pixels to disk.

Each command uses a bounded D-Bus call and makes one bounded activation/recovery
attempt. If the service remains unavailable or Session Readiness fails, it
executes `/usr/bin/cosmic-launcher alt-tab` or
`/usr/bin/cosmic-launcher shift-alt-tab` directly. It never redispatches the
overridden semantic shortcut action.
