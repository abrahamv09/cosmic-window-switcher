# COSMIC Window Switcher

COSMIC Window Switcher is a native Rust Window switcher for the COSMIC desktop.
The executable includes a resident Switcher Service plus integration probes for
Window switching and workspace-move capability verification. The service
observes Window focus metadata throughout a COSMIC Session and keeps the current
MRU Order without opening a visible Window or starting thumbnail capture. Its
quick-switch path captures the initial hold modifiers, selects in the requested
MRU direction, and activates on release without flashing an overlay. Longer
holds reveal a native icon-and-title Switcher Grid on the display containing the
initially focused Window.

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

Start the resident service from a COSMIC Wayland session:

```sh
cargo run --release -- service
```

Only one process can own the service's user-session D-Bus name. In another
terminal, inspect the current MRU Order:

```sh
cargo run --release -- status
```

`status` reports either `mru_history: accurate` or `mru_history: warm-up`. MRU
Warm-up means the service restarted with pre-existing Windows whose relative
focus history cannot be reconstructed. Opaque Window identities are shown so
the MRU Order can be verified; Window titles and pixels are never included.

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

Every card retains its installed application icon (or an
application-identity monogram), Window title, and high-contrast selected state.
A denied, stopped, failed, protected, or unsupported Window capture degrades
only that Switcher Item to its icon-and-title card. Native Wayland Windows and
compositor-managed XWayland Windows use the same capture path. The same names,
positions, focus, and selected state are exposed to assistive technology
through AT-SPI. `Tab` moves forward, `Shift+Tab` moves backward, and both
directions wrap. The grid remains above fullscreen content without changing
the selected Window's fullscreen state. Closed Windows disappear without
reordering survivors, while Windows opened during switching wait for the next
Switching Session. When all rows do not fit, rendering follows the selected row
so it remains visible.

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

## Verify workspace-move capability

First inventory the packaged compositor's advertised management protocol,
capabilities, workspace topology, Window ids, workspace selectors, and output
names:

```sh
cargo run --release -- probe-workspace-move
```

The inventory distinguishes a spanning workspace group from separate-display
workspace groups. To move one test Window to another workspace, copy the opaque
Window id and exact workspace selector from that output:

```sh
cargo run --release -- probe-workspace-move \
  --window <window-id> \
  --workspace <target-workspace-selector>
```

Add `--output <output-name>` if a target in a multi-output workspace group
cannot be resolved from the Window's current output. The probe sends exactly
one `move_to_ext_workspace` request only when the compositor advertises
management protocol v4 or newer and capability 8. It then reads the resulting
workspace membership back from COSMIC. Missing, ignored, or rejected
capabilities fail clearly without trying the unadvertised legacy path.
