Status: ready-for-agent

## Problem Statement

The stock switcher in the Pop!_OS 24.04 COSMIC desktop presents Windows as a vertical text-oriented list without live visual thumbnails. Its order does not communicate a predictable most-recently-used history, so the currently focused and previously focused Windows may appear in unexpected positions. This makes rapid keyboard switching slower and less reliable than the Windows-style Alt+Tab interaction and the earlier Pop!_OS 22.04 experience.

The user wants to identify Windows visually, return to the previous Window with one quick chord, and cycle through a stable MRU Order while holding a modifier. The replacement must behave like a native part of COSMIC: it must respect COSMIC Workspace Policy, COSMIC Accessibility Policy, COSMIC Shortcut Policy, multiple displays, fractional scaling, Native Wayland Windows, and XWayland Windows. It must also remain reversible, privacy-preserving, and safe when compositor capabilities or the resident service fail.

## Solution

Build COSMIC Window Switcher, a native Rust and libcosmic shell client distributed as an explicitly enabled Debian package for Pop!_OS 24.04. A lightweight per-user Switcher Service observes Window activation events to maintain MRU Order. When invoked, it presents a centered continuous Switcher Grid on the Session Display, initially selects the previously focused Window, displays damage-driven Live Thumbnails with icon and title, cycles while the hold modifier remains pressed, and activates the selection when that modifier is released.

The switcher will obtain Window, output, capture, and management state through COSMIC and Wayland protocols. It will use a GPU-native Capture Backend when compatible and fall back to shared memory. Its own visual and performance preferences will use `cosmic-config`, while workspace organization, accessibility activation, and shortcut assignment remain owned by COSMIC. If Session Readiness cannot be established atomically, the same Invocation Request will be delegated to the stock COSMIC switcher.

## User Stories

1. As a COSMIC user, I want each open Window represented by a Live Thumbnail, so that I can recognize it faster than by title alone.
2. As a COSMIC user, I want the currently focused Window first in MRU Order, so that the order always has an understandable anchor.
3. As a COSMIC user, I want the previously focused Window selected when the switcher opens, so that a quick Alt+Tab returns me to it.
4. As a keyboard user, I want repeated forward invocations to advance through MRU Order, so that switching remains predictable.
5. As a keyboard user, I want reverse invocation to begin at the least-recent item and move backward, so that Alt+Shift+Tab behaves naturally.
6. As a keyboard user, I want forward and reverse navigation to wrap, so that every Window remains reachable without changing direction.
7. As an Alt+Tab user, I want releasing my initial hold modifier to activate the selection, so that the interaction matches familiar Windows-style switching.
8. As a fast keyboard user, I want a quick chord release to switch without flashing the overlay, so that rapid toggling feels immediate.
9. As a user of a shortcut without Alt, Ctrl, or Super, I want Latch Mode to remain open until Enter, click, or Escape, so that arbitrary COSMIC shortcuts remain usable.
10. As a keyboard user, I want Escape to cancel without changing the Window that originally had focus, so that exploration is reversible.
11. As a user with only one Eligible Window, I want invocation to leave focus unchanged without displaying an overlay, so that the switcher does not create pointless visual noise.
12. As a workspace user, I want application Windows from every workspace included, so that I can switch without first navigating COSMIC's workspace view.
13. As a workspace user, I want selecting a Window to activate its existing workspace without relocating it, so that normal switching preserves my workspace organization.
14. As a multi-monitor user, I want Eligible Windows from all displays in one MRU Order, so that display boundaries do not fragment switching.
15. As a workspace user, I want COSMIC's workspace interface to remain responsible for organizing Windows, so that the switcher stays focused on changing between applications.
16. As a user with minimized Windows, I want them included and restored when selected, so that minimization does not make them unreachable.
17. As a user working with dialogs and utility Windows, I want independently exposed top-level Windows included, so that I can switch directly to them.
18. As a COSMIC user, I want panels, docks, menus, notifications, and overlays excluded, so that only task Windows appear.
19. As a keyboard user, I want the Session Window Set and its MRU Order to remain stable while the overlay is open, so that items do not jump beneath my selection.
20. As a user whose Window closes during switching, I want it removed without reordering survivors, so that navigation remains coherent.
21. As a user whose selected Window closes, I want selection to advance to the next surviving MRU item, so that the session remains useful.
22. As a user who opens a new Window during switching, I want it deferred until the next Switching Session, so that the current layout remains stable.
23. As a mouse user, I want hover to select only after I actually move the pointer following reveal, so that an old pointer position does not unexpectedly change selection.
24. As a mouse user, I want clicking a Switcher Item to activate its Window, so that keyboard and pointer interaction are equally direct.
25. As a mouse user, I want clicking outside the overlay to cancel, so that dismissal is intuitive.
26. As a fullscreen user, I want the overlay above fullscreen content without removing fullscreen state, so that switching has no lasting layout side effects.
27. As a multi-monitor user, I want one overlay on the display containing the initially focused Window, so that the switcher has a clear visual location.
28. As a user with many Windows, I want one continuous grid that wraps into additional rows, so that every item remains in one MRU layout without pagination.
29. As a keyboard user with many Windows, I want navigation to reveal the selected row automatically, so that off-screen items appear as I reach them.
30. As a user with different display sizes, I want small, medium, and large card presets, so that I can balance density and readability.
31. As a user comparing visual content, I want thumbnails fitted without cropping or distortion, so that their full Window contents remain recognizable.
32. As a user, I want every card to retain its application icon and Window title, so that identification remains possible when content is visually similar.
33. As a user viewing active content, I want changed Window contents refreshed up to my chosen Refresh Ceiling, so that thumbnails represent current activity.
34. As a user with many grid rows, I want continuous capture limited to the Grid Viewport, so that off-screen Windows do not consume unnecessary resources.
35. As a user navigating under GPU load, I want the selected thumbnail kept freshest, so that the Window I am considering has the best available preview.
36. As a performance-conscious user, I want 15, 30, 60, and match-display Refresh Ceiling choices, so that I can choose the resource tradeoff my machine can handle.
37. As a user without graphics-driver expertise, I want Capture Backend selection automatic, so that I do not have to understand DMA-BUF formats or modifiers.
38. As a user with an unsupported or protected Window, I want an icon-and-title fallback rather than losing that Window from MRU Order, so that switching still works.
39. As a user, I want off, light, and strong background dimming choices, so that I can tune visual focus.
40. As a motion-sensitive user, I want animations disabled when COSMIC reduced-motion policy requires it, so that the switcher respects my desktop accessibility choice.
41. As a user, I want 20, 40, 60, 80, 100, and 200 ms reveal-delay choices, so that I can tune quick switching versus accidental overlay flashes without a zero-delay animation race.
42. As a user editing preferences, I want changes saved immediately but applied from the next Switching Session, so that an open grid never reflows beneath me.
43. As a screen-reader user, I want semantic names, roles, positions, and selected state, so that I can navigate the switcher non-visually.
44. As a high-contrast user, I want the switcher to follow COSMIC high-contrast state, so that its content remains readable.
45. As an English- or Spanish-speaking user, I want the switcher to follow my COSMIC locale, so that its settings and diagnostics use my language.
46. As a user configuring shortcuts, I want COSMIC Settings to remain authoritative, so that shortcut behavior is managed in one familiar place.
47. As a user installing the package, I want shortcut replacement to require explicit enablement, so that installation alone does not alter my desktop behavior.
48. As a user disabling or uninstalling the switcher, I want only app-owned shortcut values restored or removed, so that later manual edits are preserved.
49. As a user encountering a service or capability failure, I want the stock COSMIC switcher invoked in the same direction, so that Alt+Tab remains available.
50. As a user, I want the Switcher Service resident from COSMIC login, so that it can observe focus history before the first invocation.
51. As a user after service restart, I want MRU Warm-up reported honestly and ordered deterministically, so that unknown history is not presented as random or recovered.
52. As a battery-conscious user, I want the idle service to track metadata without capturing pixels, so that it has effectively no sustained idle cost.
53. As a privacy-conscious user, I want thumbnail pixels kept only in process, GPU, or compositor memory, so that Window contents are never written to disk.
54. As a privacy-conscious user, I want lock, suspend, user switch, and session shutdown to destroy overlays and capture buffers immediately, so that previews cannot cross a session boundary.
55. As a privacy-conscious user, I want no telemetry or automatic crash upload, so that usage data never leaves my machine.
56. As a user requesting diagnostics, I want default logs to exclude Window titles and pixels, so that troubleshooting is privacy-safe.
57. As a COSMIC user running mixed applications, I want Native Wayland Windows and XWayland Windows handled together, so that legacy applications and games do not disappear.
58. As a user logging into GNOME, Ubuntu, Xorg, or another desktop, I want the switcher to refuse activation cleanly, so that COSMIC-specific integration does not damage another session.
59. As a user receiving COSMIC updates, I want runtime capability negotiation instead of an exact desktop-version lock, so that compatible newer releases continue working.
60. As a Pop!_OS user, I want an `amd64` Debian package, so that installation and removal use the operating system's native package tooling.
61. As a release consumer, I want a checksum and maintainer signature for the package, so that I can verify the artifact downloaded from GitHub Releases.
62. As the maintainer, I want release candidates tested on both Pop!_OS machines, so that the package is validated beyond one development environment.
63. As a frequent switcher user, I want selection feedback within one display frame and the overlay ready within its latency target, so that the replacement feels native.
64. As a user under capture overload, I want keyboard and pointer input prioritized over thumbnail freshness, so that the switcher never feels stuck.
65. As a user configuring the switcher, I want a standalone native settings window, so that v1 remains configurable despite COSMIC Settings not supporting external pages.
66. As a user, I want the stock COSMIC switcher left installed and directly callable, so that recovery never depends on the custom semantic shortcut mapping.

## Implementation Decisions

- The product is COSMIC Window Switcher. Domain language distinguishes a Window from an application and a Switcher Item from an application group.
- V1 is an unsandboxed native Rust and libcosmic Wayland client. Current COSMIC filters required Window-management and capture globals from sandboxed clients.
- V1 supports only compatible COSMIC Wayland sessions on Pop!_OS 24.04 `amd64`. Native Wayland Windows and compositor-managed XWayland Windows share the same user-visible behavior.
- The product ships one `cosmic-window-switcher` executable with `service`, `invoke next|previous`, `settings`, `enable|disable`, and `status|doctor` modes.
- A resident per-user Switcher Service owns the compositor connection, Window registry, MRU Order, capture resources, and active Switching Session.
- Short-lived commands communicate with the Switcher Service through a narrow versioned user-session D-Bus interface. D-Bus messages never carry Window pixels or titles.
- Runtime code is organized around a pure switching domain, a compositor adapter, a capture adapter and scheduler, an overlay renderer/input adapter, configuration, lifecycle integration, and diagnostics.
- The switching domain accepts typed Window, input, time, preference, and session events and emits typed overlay, activation, capture, and fallback effects. It does not expose Wayland, renderer, D-Bus, or filesystem types.
- The Window registry uses opaque stable identities supplied by the compositor and tracks title, application identity, state, output membership, workspace membership, and lifecycle.
- MRU Order is derived from observed activation transitions. During MRU Warm-up, the current Window is first and unknown survivors retain deterministic discovery order until actual focus events establish their relative recency.
- A Switching Session snapshots its Session Window Set, MRU Order, Session Display, and Session Preferences at invocation. Newly created Windows are deferred, while closed Windows are removed without reordering survivors.
- Forward invocation initially selects the second MRU item. Reverse invocation initially selects the final item. One item is a no-op, and both directions wrap.
- Hold Mode captures initially held Alt, Ctrl, or Super modifiers and commits when the last of them is released. Shift changes direction but does not keep the session open. Invocation with no hold modifier uses Latch Mode.
- The service maps an initially transparent exclusive-keyboard layer surface promptly so modifier release cannot be missed. The visible Switcher Grid is revealed only after atomic Session Readiness; a quick release before reveal commits without a flash.
- Session Readiness requires compatible compositor globals, Window control, temporary keyboard focus, renderer resources, and a usable icon/title fallback for every initial item. It does not wait for every live frame.
- Failure before visible reveal destroys partial resources and executes the stock COSMIC launcher directly in the original direction. Redispatching the overridden semantic shortcut action is forbidden because it would recurse.
- The Switcher Grid is one centered continuous fixed-card layout in MRU Order. It wraps vertically, has no pages, and scrolls just enough to reveal the selected row.
- Pointer hover is ignored until post-reveal movement. Click activates and outside click cancels.
- Live Thumbnail capture uses the ext foreign-toplevel image source and image-copy-capture protocol family. Frames use exact compositor-reported source dimensions, one outstanding request per stream, damage-driven refresh, and explicit stopped/failed handling.
- The Capture Backend prefers DMA-BUF only when compositor formats, device, modifiers, allocator, and presentation import are compatible. Shared memory is the correctness fallback and is sufficient for Session Readiness.
- Rendering fits full Window content inside the card without cropping or distortion. Icon and title remain visible in normal and degraded states.
- Only rows intersecting the Grid Viewport continue capture. Scheduling prioritizes input, then the selected item, then fair round-robin work for other visible items. The Refresh Ceiling throttles changed content and does not request duplicate frames for unchanged Windows.
- Individual capture denial or failure degrades only that Switcher Item. Missing capture protocols for the session trigger atomic stock fallback because live thumbnails are a core product requirement.
- All Workspaces is the default Window Scope, so every independently exposed application Window participates in one global MRU Order. Selecting a Window uses normal COSMIC activation to follow it to its existing workspace without relocating it. Visible Workspaces remains a capability-gated stricter future scope.
- Workspace organization remains owned by COSMIC's existing workspace interface. The switcher does not show a workspace view, expose workspace targets, issue workspace-move requests, or depend on a workspace-move capability.
- Switcher Preferences are a versioned typed `cosmic-config` schema containing card size, dimming, Refresh Ceiling, animation, and reveal delay. Missing or invalid values fall back safely, and future versions migrate deliberately.
- COSMIC Workspace Policy, COSMIC Accessibility Policy, and COSMIC Shortcut Policy remain external authoritative inputs and are never duplicated into Switcher Preferences.
- The settings window is a standalone libcosmic application in v1 because external COSMIC Settings pages are not currently supported.
- Explicit enablement changes only the user-level semantic commands for `WindowSwitcher` and `WindowSwitcherPrevious`. It records prior values transactionally and restores/removes a value only if its current value still matches the app-owned command.
- Existing COSMIC key-to-action mappings remain untouched, allowing COSMIC Settings to control which keys invoke switching.
- The service starts with the COSMIC user session so it can observe activation history. D-Bus activation remains available for recovery and single-instance behavior.
- Lock, suspend, user switch, compositor loss, and session shutdown cancel without Window activation, destroy capture and overlay resources, and pause MRU tracking until a valid COSMIC session resumes.
- Capability negotiation checks required protocol names, versions, and advertised operations at runtime. Packaging declares known minimum dependencies without requiring one exact COSMIC release.
- Diagnostics are structured and local. Default output may include versions, capabilities, service state, Capture Backend, shortcut ownership, and MRU Warm-up state, but not Window titles or pixels.
- The package is GPL-3.0-only, owned by Abraham Velazquez, and uses application ID `io.github.abrahamv09.CosmicWindowSwitcher`.
- English and Spanish use Fluent resources and follow the desktop locale without a competing language setting.
- GitHub Releases distribute a checksummed and maintainer-signed Debian package. Package installation is inert until explicit enablement.

## Testing Decisions

- Good tests assert behavior visible at an approved seam: resulting selection, overlay model, compositor effect, fallback command, persisted preference, shortcut ownership, or diagnostic result. Tests do not assert private helper calls, internal collection shapes, or incidental protocol-object layout.
- The primary seam is a service-scenario harness. It sends Invocation Requests plus typed compositor, input, time, session, and preference events to a running Switcher Service and observes grid state and emitted effects through fake external adapters.
- The service-scenario seam covers MRU Order, MRU Warm-up, eligibility, cross-workspace activation, forward/reverse wrapping, Hold Mode, Latch Mode, reveal delay, cancellation, stable Session Window Set behavior, Window closure, preference snapshots, continuous-grid selection reveal, visible-row capture scheduling, Session Deactivation, and fallback.
- The second seam is a live COSMIC contract probe. It verifies the real compositor globals and event ordering for Window enumeration, Native Wayland/XWayland identity, activation, minimized restore, optional workspace/output membership, exclusive keyboard focus, modifier release, SHM capture, optional DMA-BUF capture, and fullscreen overlay behavior.
- The live contract probe classifies capabilities as required or optional. Failure of Window enumeration/state, activation, exclusive keyboard behavior, or shared-memory capture blocks the architecture. Missing DMA-BUF or workspace membership is compatible with the default All Workspaces scope.
- The third seam is a lifecycle sandbox with isolated XDG configuration, user-session D-Bus, and service state. It verifies fresh enablement, existing custom semantic commands, transactional rollback, user edits after enablement, repeated enable/disable, service recovery, stock fallback without recursion, upgrade, uninstall, and unsupported-session rejection.
- Capture tests use fake damage events, exact buffer constraints, backend negotiation, frame completion/failure, viewport changes, and a deterministic clock. They assert scheduling fairness, input priority, resource release, and absence of pixel persistence.
- Layout tests assert observable card geometry, MRU traversal, selected-row reveal, aspect-ratio fit, scale handling, title truncation, and accessibility state across card sizes, Window counts, and display geometries.
- Configuration tests cover defaults, invalid values, schema migration, immutable Session Preferences, and change notification without depending on raw storage implementation.
- Accessibility checks cover semantic roles, accessible names, selected position, high contrast, reduced motion, focus order, and screen-reader announcements.
- Privacy tests assert that session end, lock, suspend, user switch, failure, and Window closure release every buffer and that default logs and D-Bus messages contain neither pixels nor Window titles.
- Performance tests use the development machine as the reference: effectively zero sustained idle CPU, selection feedback within one display frame, visible overlay ready within 50 ms after the configured delay, and smooth default 30 FPS changed-content handling with ten visible Windows while input remains responsive.
- Release validation repeats the manual matrix on the development laptop and the MSI Aegis ZS2, including Native Wayland and representative XWayland applications, fullscreen, minimization, multiple workspace policies, available multi-monitor layouts, fractional scaling, DMA-BUF, forced shared-memory fallback, install, upgrade, disable, and uninstall.
- This is a greenfield codebase, so there are no existing repository tests to imitate. Primary prior art is the stock COSMIC launcher's hold/release behavior, official COSMIC protocol handlers, libcosmic configuration patterns, and the approved protocol contract probe.

## Out of Scope

- Closing a Window from its Switcher Item in v1.
- A dedicated minimize/restore control in v1; selecting an already minimized Window still restores it.
- Workspace views, workspace targets, or relocating Windows; COSMIC's workspace interface owns workspace organization.
- Embedding a page inside COSMIC Settings until an upstream external-page mechanism exists.
- Replacing or forking `cosmic-comp`, `cosmic-launcher`, or `cosmic-settings`.
- Supporting GNOME, the Ubuntu session, KDE, an Xorg desktop session, or another Wayland compositor.
- Flatpak or another sandboxed distribution format in v1.
- Architectures other than `amd64` in v1.
- An apt repository, automatic updater, telemetry, or automatic crash upload.
- Persisting Window pixels, screenshots, or thumbnail caches.
- A user-selectable capture transport or raw driver settings.
- Multiple duplicated overlays across displays.
- Paginated grids, application grouping, or Windows visual imitation.
- Guaranteed duplicate frames for unchanged Windows or a guaranteed Refresh Ceiling under resource overload.

## Further Notes

- Core feasibility is supported by first-party COSMIC and Wayland sources: Window enumeration/state, activation, per-Window image capture, SHM transport, optional DMA-BUF, layer-shell exclusive keyboard behavior, workspace/output observation, `cosmic-config`, and semantic shortcut overrides are available to an unsandboxed client.
- Workspace movement is not a product requirement. COSMIC's workspace interface owns workspace organization, so the compositor's mismatched external move capability does not block v1.
- DMA-BUF presentation through the evolving libcosmic/iced subsurface path needs an early integration probe. Shared memory is the required correctness path and permits the rest of the switcher to proceed if DMA-BUF is unavailable.
- The resident service cannot reconstruct historical MRU Order for pre-existing Windows after a cold or crash restart. MRU Warm-up makes this limitation deterministic and explicit until new focus events rebuild history.
- Version 2 is the planned home for close controls and explicit minimize/restore controls.
