# COSMIC integration surface for the app switcher

Status: researched 2026-07-29  
Target: Pop!_OS 24.04 LTS, COSMIC Wayland session  
Scope: v1 feasibility and integration boundaries

## Executive conclusion

A native, Windows-style switcher is feasible as an **unsandboxed COSMIC Wayland client plus a resident user-session service**. Current COSMIC and upstream Wayland protocols provide the essential primitives: enumerate windows, observe activation and workspace state, activate or close a window, capture each window into SHM or DMA-BUF buffers, and show an exclusive-keyboard overlay. COSMIC's own launcher demonstrates the difficult Alt-release interaction.

The recommended v1 shape is:

1. One packaged Rust executable with `service`, `invoke`, `settings`, `enable`, and `disable` modes.
2. A resident service started with `cosmic-session.target` so it can build an MRU history from activation changes.
3. A `libcosmic` overlay using layer-shell with exclusive keyboard interactivity.
4. `ext-image-copy-capture-v1`, sourced from foreign toplevels, with DMA-BUF when available and SHM as the mandatory practical fallback.
5. Runtime protocol negotiation and an atomic fallback to the stock COSMIC switcher when the required surface is unavailable.
6. COSMIC Settings remains the authority for key bindings. Enabling this switcher changes only the two semantic system-action commands, with ownership-aware restoration.
7. A standalone settings UI for v1. COSMIC Settings' current source explicitly leaves external plugins unsupported.

There is one material blocker for the proposed v1 mouse drag-to-workspace feature: current upstream `cosmic-comp` advertises the legacy workspace-move capability whose request it ignores, while it implements the new ext-workspace request without advertising that capability. The feature must be runtime-gated until the installed compositor is verified to behave differently, or COSMIC accepts/fixes the upstream capability mismatch.

## Evidence vocabulary

- **Proven** means a protocol contract and/or current first-party implementation directly supplies the behavior.
- **Inference** means the design follows from proven primitives but still needs an integration prototype or device testing.
- **Blocker/risk** means the contract is absent, contradictory, optional, or not observable on the audited installation.

## Audited system baseline

The audited host is Pop!_OS 24.04 LTS on an active COSMIC Wayland session. `cosmic-comp` is the compositor and an embedded rootless XWayland server is running. The installed session package depends on both `cosmic-comp` and `xwayland`. Only `/usr/share/wayland-sessions/cosmic.desktop` is installed; the failed “Ubuntu” login reported by the user is not a supported secondary desktop stack on this machine.

Relevant installed package/build observations were:

| Component | Audited package/build |
| --- | --- |
| `cosmic-comp` | package `0.1~1785277801~24.04~ffeda33`; binary reports `1.0.0` |
| `cosmic-session` | `1.0.0~1783021552~24.04~b5ef6c0` |
| `cosmic-settings` | `1.0.12~1785277759~24.04~7287257` |
| `cosmic-settings-daemon` | `0.1.0~1784740199~24.04~21a9692` |
| `cosmic-launcher` | `1.0.12~1785249651~24.04~8799503` |
| XWayland | `2:24.1.12-1pop2~...` |
| Rust toolchain | `rustc`/Cargo 1.95 |

The installed `cosmic-comp` binary contains the names of the required foreign-toplevel, image-capture, layer-shell, and workspace globals. This is strong evidence that the Pop build includes them, but the host lacks `wayland-info` and installed protocol XML, so the exact versions advertised at runtime were not measured. Runtime version/capability checks remain mandatory.

There are no corresponding installed COSMIC development packages for this client stack. The application should pin exact Rust git/crate revisions and build them into the package rather than assume a stable system ABI.

## Capability matrix

| Requirement | Status | Integration surface |
| --- | --- | --- |
| Enumerate Wayland and XWayland windows | **Proven** | `ext_foreign_toplevel_list_v1` plus `zcosmic_toplevel_info_v1` |
| Title, app ID, active/minimized/fullscreen/sticky state | **Proven** | COSMIC toplevel-info events |
| Stable MRU ordering | **Inference** | Resident tracker; no protocol supplies activation history |
| Activate and close | **Proven** | `zcosmic_toplevel_manager_v1` |
| Move to an ext workspace | **Blocker/risk** | Request is implemented, but capability advertisement is inconsistent |
| Capture individual window contents | **Proven** | ext foreign-toplevel image source + ext image-copy-capture |
| DMA-BUF fast path | **Proven, optional** | Advertised only when compositor renderer/device supports it |
| SHM fallback | **Proven** | Current `cosmic-comp` advertises SHM formats |
| Alt-hold/Tab-cycle/Alt-release | **Proven** | layer-shell exclusive keyboard; stock launcher demonstrates it |
| Respect per-display/spanning workspace policy | **Proven** | ext-workspace groups, outputs, active state, toplevel membership |
| Persist preferences like a COSMIC app | **Proven** | `cosmic-config` |
| Preserve COSMIC shortcut configuration | **Proven** | semantic `WindowSwitcher` system-action override |
| Integrate a page into COSMIC Settings | **Blocked upstream** | current source says external plugins are unsupported |
| Native COSMIC rendering | **Proven** | `libcosmic`; iced Wayland subsurface buffers are promising for thumbnails |
| Work in X11 desktop sessions | Out of scope | package should require the COSMIC Wayland session |

## Window discovery, state, and MRU

The standard ext foreign-toplevel list enumerates mapped desktop windows and exposes title, app ID, and identifier. Its specification allows compositors to expose XWayland windows through the same abstraction. COSMIC's extension adds output membership, geometry, active/minimized/maximized/fullscreen/sticky state, and, at protocol version 3, ext-workspace membership. See the [standard foreign-toplevel list specification](https://gitlab.freedesktop.org/wayland/wayland-protocols/-/blob/main/staging/ext-foreign-toplevel-list/ext-foreign-toplevel-list-v1.xml) and [COSMIC toplevel-info protocol](https://github.com/pop-os/cosmic-protocols/blob/e95d89504513e1407f89a189aca328fbecc9eeef/unstable/cosmic-toplevel-info-unstable-v1.xml).

`cosmic-comp` advertises these globals only to clients it considers unsandboxed. That makes a normal `.deb` appropriate and makes a conventional Flatpak unsuitable without a COSMIC-specific permission path. The filter is visible in [`state.rs`](https://github.com/pop-os/cosmic-comp/blob/091583ac84abac02967ae358cf9570ddfef63b31/src/state.rs).

**Proven:** mapped and minimized windows remain discoverable until they are genuinely unmapped/closed. Activation, state, outputs, and workspace membership can be tracked continuously.

**Inference:** neither protocol contains MRU history. A resident process must timestamp activation transitions. On a cold service start, the current active window is known but older order is unknowable; use a deterministic fallback (for example current window, then protocol discovery order) until real activation history accumulates. This limitation should be documented rather than presented as exact MRU immediately after installation or service restart.

## Window management and the workspace-move mismatch

COSMIC's management protocol defines activation, close, maximize, minimize, fullscreen, sticky, and workspace move requests; the compositor is permitted to ignore a request. Activation takes a seat and current `cosmic-comp` unminimizes the window, activates its workspace, and gives it focus. See the [management protocol](https://github.com/pop-os/cosmic-protocols/blob/e95d89504513e1407f89a189aca328fbecc9eeef/unstable/cosmic-toplevel-management-unstable-v1.xml) and [compositor handler](https://github.com/pop-os/cosmic-comp/blob/091583ac84abac02967ae358cf9570ddfef63b31/src/wayland/handlers/toplevel_management.rs).

**Blocker/risk:** protocol version 4 deprecates the old `move_to_workspace` request in favor of `move_to_ext_workspace`. At current upstream HEAD:

- compositor setup advertises `MoveToWorkspace`, the legacy capability, in [`state.rs`](https://github.com/pop-os/cosmic-comp/blob/091583ac84abac02967ae358cf9570ddfef63b31/src/state.rs);
- dispatch explicitly ignores the legacy request;
- dispatch accepts the new `MoveToExtWorkspace` request, but setup does not advertise its corresponding capability.

The contradictory dispatcher is visible in [`toplevel_management.rs`](https://github.com/pop-os/cosmic-comp/blob/091583ac84abac02967ae358cf9570ddfef63b31/src/wayland/protocols/toplevel_management.rs).

A capability-respecting client therefore cannot reliably promise drag-to-workspace against this revision. Do not silently issue an unadvertised request in production. Gate the v1 UI on an end-to-end probe or a corrected advertised capability, and file/track the upstream mismatch. If the fix cannot land for v1, retain workspace visualization but defer the actual drop action.

## Live thumbnail capture

The standard image-capture-source protocol can create a source from an ext foreign toplevel; image-copy-capture then reports buffer constraints and captures that source. The relevant primary specifications are [ext-image-capture-source-v1](https://gitlab.freedesktop.org/wayland/wayland-protocols/-/blob/main/staging/ext-image-capture-source/ext-image-capture-source-v1.xml) and [ext-image-copy-capture-v1](https://gitlab.freedesktop.org/wayland/wayland-protocols/-/blob/main/staging/ext-image-copy-capture/ext-image-copy-capture-v1.xml).

Important operational constraints:

- Buffers must match the source's exact reported size.
- A session reports SHM formats and may report DMA-BUF formats/modifiers and a device.
- Only one frame may be outstanding per session.
- Subsequent requests are damage-driven and may remain pending until the source changes.
- The compositor may stop a session or fail an individual frame.

Current `cosmic-comp` creates exact-size constraints and advertises SHM formats. It also advertises DMA-BUF formats when an EGL render node and compatible modifiers exist. Its SHM path still renders off-screen on the GPU and copies/maps the result to client memory, so DMA-BUF is the preferred fast path but not a v1 correctness requirement. The compositor implementation is in [its image-copy-capture handler](https://github.com/pop-os/cosmic-comp/blob/091583ac84abac02967ae358cf9570ddfef63b31/src/wayland/handlers/image_copy_capture/mod.rs); COSMIC's client toolkit exposes both the formats and damage model in [its screencopy module](https://github.com/pop-os/cosmic-protocols/blob/e95d89504513e1407f89a189aca328fbecc9eeef/client-toolkit/src/screencopy/mod.rs).

**Proven:** per-window contents and damage-driven refresh are available to an unsandboxed client. A user-selected 30-fps ceiling can limit request cadence, but it cannot require unchanged windows to produce 30 identical frames; “live” should mean refresh on damage up to the ceiling.

**Inference/risk:** source buffers are full window resolution even when displayed as small cards. Limit active capture sessions or refresh work to visible/near-visible rows, retain the last successful buffer, and scale at presentation. A single window's frame failure should degrade that card to cached content or icon/title. If the required capture globals are missing at invocation startup, use the stock switcher atomically instead of revealing a half-functional overlay.

## Overlay keyboard behavior

The [wlr layer-shell protocol](https://gitlab.freedesktop.org/wlroots/wlr-protocols/-/blob/master/unstable/wlr-layer-shell-unstable-v1.xml) provides overlay-layer surfaces and exclusive keyboard interactivity. Exclusive mode gives keyboard focus to the topmost eligible layer surface; it is not a privileged global keyboard grab.

COSMIC Launcher already implements the exact interaction needed: it creates an exclusive-keyboard layer surface, handles Tab and Escape, observes Alt release, and defers activation if the modifier is released before the launcher becomes visible. See [`cosmic-launcher/src/app.rs`](https://github.com/pop-os/cosmic-launcher/blob/585a8c0c98d0385a91942c7f0e54d7ab209c1e79/src/app.rs).

**Proven:** hold Alt, cycle with Tab/Shift+Tab, activate on Alt release is implementable in a normal unsandboxed client.

**Inference:** to reconcile reliable release detection with “reveal only when ready,” map an initially transparent exclusive-keyboard layer surface promptly, track modifier state, and reveal the cards only after the immutable invocation snapshot and minimum thumbnail/fallback state are ready. Waiting for every live thumbnail before mapping the keyboard surface risks missing Alt release.

## Workspaces, outputs, and COSMIC policy

The [ext-workspace protocol](https://gitlab.freedesktop.org/wayland/wayland-protocols/-/blob/main/staging/ext-workspace/ext-workspace-v1.xml) exposes workspace groups, the outputs assigned to each group, workspace active/hidden state, and coordinates. COSMIC's [workspace extension](https://github.com/pop-os/cosmic-protocols/blob/e95d89504513e1407f89a189aca328fbecc9eeef/unstable/cosmic-workspace-unstable-v2.xml) adds COSMIC-specific tiling, pinning, and reordering features. Toplevel-info v3 connects each window to ext workspace handles.

That protocol state is a better runtime authority than duplicating the Settings app's “workspaces span displays” versus “displays have separate workspaces” preference:

- workspace groups and their output sets express spanning versus per-output organization;
- active workspaces define what is visible now;
- each toplevel's workspace/output events determine inclusion;
- workspace coordinates provide ordering for workspace drop targets.

**Proven:** the switcher can respect current workspace/output behavior without superseding COSMIC's settings.

**Inference/risk:** the protocol supplies coordinates but does not label an axis “vertical” or “horizontal.” Derive ordering from coordinates; if the visual orientation must exactly echo the Settings control, test COSMIC's emitted coordinate convention or read the corresponding COSMIC config as a presentation hint. The window-filtering semantics should still come from live protocol state.

## Configuration and settings UI

`cosmic-config` stores each field as RON beneath `$XDG_CONFIG_HOME/cosmic/<application-id>/vN` and discovers packaged defaults under the XDG data directories, conventionally `/usr/share/cosmic/<application-id>/vN`. It supports typed derived entries, atomic transactions, versioned fallback, and change subscriptions. See [`cosmic-config`](https://github.com/pop-os/libcosmic/blob/dc1cf9f00cbe2902a52166492654bb9fee8a73d1/cosmic-config/src/lib.rs) and its [derive support](https://github.com/pop-os/libcosmic/blob/dc1cf9f00cbe2902a52166492654bb9fee8a73d1/cosmic-config-derive/src/lib.rs).

Use application ID `io.github.abrahamv09.CosmicWindowSwitcher`, schema version 1, and install explicit defaults. The service may watch for changes, but each switcher invocation should take one immutable settings snapshot so layout/order cannot shift midway through Alt+Tab.

For v1, ship a standalone `libcosmic` settings window. Current COSMIC Settings reaches `todo!("external plugins not supported yet")` for external plugins in [`app.rs`](https://github.com/pop-os/cosmic-settings/blob/823ef166f96330bda4f530aaf8395c9b054d3eec/cosmic-settings/src/app.rs). Direct integration should remain a later upstream-coordinated option, not a packaging assumption.

## Keyboard shortcuts and safe stock fallback

COSMIC's shortcut schema is `com.system76.CosmicSettings.Shortcuts`, version 1. Bindings map keys to semantic actions; a separate `system_actions` map resolves actions such as `WindowSwitcher` and `WindowSwitcherPrevious` to commands. User maps override system defaults. See the settings daemon's [shortcut merge logic](https://github.com/pop-os/cosmic-settings-daemon/blob/21a9692b53fcbffa0f18f7d0a12bf0f9d5bd0590/config/src/shortcuts/mod.rs), [action types](https://github.com/pop-os/cosmic-settings-daemon/blob/21a9692b53fcbffa0f18f7d0a12bf0f9d5bd0590/config/src/shortcuts/action.rs), and the compositor's [shortcut execution](https://github.com/pop-os/cosmic-comp/blob/091583ac84abac02967ae358cf9570ddfef63b31/src/input/actions.rs).

The audited defaults map Alt+Tab and Super+Tab to `WindowSwitcher`, their shifted forms to `WindowSwitcherPrevious`, and resolve those actions to `cosmic-launcher alt-tab` and `cosmic-launcher shift-alt-tab`.

Recommended lifecycle:

1. Package installation does not alter user shortcuts.
2. Explicit `enable` records whether local user `system_actions` entries existed and their exact prior values.
3. It atomically sets only the two semantic actions to this app's `invoke next` and `invoke previous` commands.
4. `disable` restores/removes an entry only when its current value still exactly matches the app-owned command. If the user changed it meanwhile, leave it alone.
5. The emergency fallback directly executes `/usr/bin/cosmic-launcher alt-tab` or `/usr/bin/cosmic-launcher shift-alt-tab`. It must not redispatch the semantic system action, which would recurse through the override.

This approach lets COSMIC Settings continue to own which keys invoke the two semantic actions. Users who want unrelated custom bindings can create explicit `Spawn(...)` shortcuts in COSMIC Settings. The app may offer a shortcut status/help page, but should not maintain a competing keybinding database.

## Session service, D-Bus, and packaging

COSMIC's session starts `cosmic-session.target` after the compositor and settings daemon are ready. The compositor imports `WAYLAND_DISPLAY` and `DISPLAY` into the user systemd environment before session readiness. See the compositor's [user unit](https://github.com/pop-os/cosmic-comp/blob/091583ac84abac02967ae358cf9570ddfef63b31/data/cosmic-comp.service), [environment handoff](https://github.com/pop-os/cosmic-comp/blob/091583ac84abac02967ae358cf9570ddfef63b31/src/systemd.rs), and [COSMIC session startup](https://github.com/pop-os/cosmic-session/blob/b5ef6c0c0d68762b2991e4f5906cc70599e2f1fc/src/main.rs).

Use a user unit approximately shaped as:

```ini
[Unit]
PartOf=cosmic-session.target
After=cosmic-comp.service

[Service]
Type=dbus
BusName=io.github.abrahamv09.CosmicWindowSwitcher
ExecStart=/usr/bin/cosmic-window-switcher service
Restart=on-failure

[Install]
WantedBy=cosmic-session.target
```

Pair it with a D-Bus activation file using the same well-known name. D-Bus activation provides single-instance invocation recovery; target startup is still necessary because a service launched only on the first Alt+Tab cannot know earlier MRU history.

Keep the D-Bus API narrow: forward invocation, reverse invocation, settings/show, and status are enough for v1. The executable should verify that it is in a COSMIC Wayland session and that required globals exist, then exit cleanly elsewhere. Enabling/disabling the service is an explicit user action; the unit stops with the COSMIC graphical session.

## XWayland behavior

`cosmic-comp` represents native Wayland and X11 windows with one `CosmicSurface` abstraction. X11 mapping feeds the same toplevel-info machinery, with title/app ID derived from X11 title/class, and management operates on that common surface. See [`surface.rs`](https://github.com/pop-os/cosmic-comp/blob/091583ac84abac02967ae358cf9570ddfef63b31/src/shell/element/surface.rs) and [`xwayland.rs`](https://github.com/pop-os/cosmic-comp/blob/091583ac84abac02967ae358cf9570ddfef63b31/src/xwayland.rs).

**Proven:** enumeration, state, activation, and close include compositor-managed XWayland windows.

**Inference/risk:** image capture uses the same common surface, but capture constraints require an associated `wl_surface`. Normal rootless XWayland windows should have one; test representative legacy/Electron/game windows on both target machines. If a particular XWayland surface cannot be captured, keep it in MRU and render its card with icon/title rather than dropping it.

Supporting an entirely separate X11 desktop session is unnecessary and should not be claimed. The product target is the COSMIC Wayland session with COSMIC-managed XWayland clients.

## `libcosmic` rendering path

`libcosmic` is the correct shell UI toolkit for COSMIC theme, scaling, accessibility, and layer surfaces. The pinned iced revision also contains a Wayland subsurface widget accepting SHM or DMA-BUF descriptors, creating buffers on iced's Wayland connection, tracking release, and scaling them with viewporter. See [`subsurface_widget.rs`](https://github.com/pop-os/iced/blob/7346cffd2e51e45fbe4dd31bdd42211b8ca0078e/winit/src/platform_specific/wayland/subsurface_widget.rs) and its [subsurface example](https://github.com/pop-os/iced/blob/7346cffd2e51e45fbe4dd31bdd42211b8ca0078e/examples/sctk_subsurface_gst/src/main.rs).

**Inference/risk:** this is a promising low-copy presentation path, not yet a proven end-to-end capture integration. Capture and iced may use different Wayland connections; the app can duplicate the underlying DMA-BUF/SHM file descriptors and create a presentation buffer on the UI connection, but must prototype synchronization, modifier compatibility, buffer release/lifetime, fractional scaling, damage, and performance with many visible subsurfaces. Pin the exact `libcosmic`/iced revisions because this is an evolving Rust API, not a stable distribution ABI.

An initial SHM implementation is a valid correctness milestone. DMA-BUF should be selected automatically only after its advertised device/formats/modifiers intersect the allocator and presentation path.

## V1 implementation gates

Before calling v1 complete:

- Verify advertised global versions and management capabilities at runtime on both Pop!_OS machines.
- Build the MRU tracker and explicitly test cold-start degradation.
- Prototype one native Wayland and one XWayland capture through SHM, then DMA-BUF.
- Verify the hidden-first exclusive layer surface never loses Alt-release.
- Test separate-display workspaces and spanning workspaces from live protocol state.
- Resolve or gate the workspace-move capability mismatch.
- Test ownership-safe shortcut enable, user edits after enable, disable, reinstall, and stock fallback without recursion.
- Validate screen-reader labels/focus order, reduced motion, high contrast, scaling, multi-monitor placement, and keyboard-only behavior.
- Keep the switcher atomic: either the custom invocation reaches its minimum ready state, or the stock launcher handles that invocation; never show a partially initialized hybrid.

## Final classification

The core reasons for this project—live thumbnails and stable MRU ordering—are supported. Thumbnail capture is a first-class Wayland/COSMIC protocol path, while MRU is deliberately application-maintained. Native activation, XWayland inclusion, layer-shell keyboard behavior, workspace visibility, COSMIC configuration, and shortcut delegation are all viable.

The unresolved engineering questions are bounded: DMA-BUF-to-iced buffer integration, exact XWayland capture coverage, cold-start MRU behavior, runtime protocol versions on packaged builds, and the upstream workspace-move capability contradiction. Of these, only workspace movement blocks a promised v1 feature; it does not block the core switcher.
