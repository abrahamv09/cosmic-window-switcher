# COSMIC Window Switcher

COSMIC Window Switcher is a native Rust Window switcher for the COSMIC desktop.
The current executable contains the ticket 1 integration probe: it proves
Window discovery, memory-only shared-memory capture, exclusive keyboard input,
and Window activation against a live COSMIC compositor.

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
