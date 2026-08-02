# Window Switching

This context defines the language for selecting an open desktop window through a temporary keyboard-driven interface.

## Language

**COSMIC Window Switcher**:
The product: a temporary interface for choosing among open windows while a switching shortcut is held.
_Avoid_: App switcher, application switcher

**COSMIC Session**:
A Wayland desktop session hosted by the COSMIC compositor. The COSMIC Window Switcher runs only in this environment and rejects GNOME, Ubuntu, and other non-COSMIC sessions as unsupported.
_Avoid_: Linux desktop session, generic Wayland session

**Window**:
One independently switchable top-level desktop window, including a dialog or utility window that COSMIC exposes independently. Non-task shell surfaces such as panels, docks, menus, notifications, and overlays are excluded.
_Avoid_: App, application

**Native Wayland Window**:
A Window whose application connects directly to the COSMIC Wayland compositor.
_Avoid_: COSMIC-only window

**XWayland Window**:
A legacy X11 Window presented through COSMIC's XWayland compatibility layer. It is fully eligible for switching even when its identity metadata or capture behavior differs from a Native Wayland Window.
_Avoid_: X11 session, unsupported window

**Switcher Item**:
The visual representation of exactly one Window inside the COSMIC Window Switcher, containing its Live Thumbnail, application icon, and Window title.
_Avoid_: App item, application item

**MRU Order**:
The Window ordering from most recently focused to least recently focused, with the currently focused Window first.
_Avoid_: App order, arbitrary order

**MRU Warm-up**:
The degraded interval after the Switcher Service starts with pre-existing Windows and has no historical activation events for them. The current Window is first and unknown survivors keep deterministic discovery order; each later activation promotes a Window into accurate MRU Order.
_Avoid_: Random order, restored history

**Switcher Service**:
The single resident per-user process that owns the COSMIC compositor connection, tracks MRU Order, and runs Switching Sessions. Shortcut commands and settings communicate with it through the user-session D-Bus.
_Avoid_: Background app, capture daemon

**Command Surface**:
The subcommands of the single installed `cosmic-window-switcher` executable: `service`, `invoke next|previous`, `settings`, `enable|disable`, and `status|doctor`. Each invocation has one purpose while sharing the package version and implementation.
_Avoid_: Helper executables, unversioned scripts

**Invocation Request**:
A small D-Bus command sent to the Switcher Service to begin or advance forward or reverse switching. It carries no thumbnail pixels or Window contents.
_Avoid_: Keyboard event, direct compositor command

**Session Readiness**:
The atomic precondition for revealing a Switcher Grid: the Switcher Service has the required compositor protocols, temporary keyboard focus, Window control, and rendering resources. Failure delegates the Invocation Request to COSMIC's stock switcher instead of exposing a partial overlay.
_Avoid_: Best-effort startup, partially ready overlay

**Capability Contract**:
The required Wayland protocol versions and runtime features that establish compatibility with a COSMIC Session. The package declares known minimum dependencies but does not require an exact COSMIC release; compatible newer releases pass capability negotiation.
_Avoid_: Exact desktop version, assumed compatibility

**Initial Selection**:
The previously focused Window, shown as the second Switcher Item when the COSMIC Window Switcher opens.
_Avoid_: Current window selection, first item selection

**COSMIC Workspace Policy**:
COSMIC's authoritative rules for whether workspaces span displays or belong to separate displays. Visible Workspaces consumes this live policy; All Workspaces does not copy or reinterpret it.
_Avoid_: Copied workspace policy, workspace override

**COSMIC Accessibility Policy**:
COSMIC's authoritative screen-reader and high-contrast state. The COSMIC Window Switcher always provides accessibility semantics and does not define a competing accessibility mode.
_Avoid_: Switcher accessibility mode, accessibility override

**COSMIC Shortcut Policy**:
COSMIC's authoritative mapping from key combinations to switcher invocation commands. The COSMIC Window Switcher may install a reversible recommended preset but does not define a competing shortcut store or recorder.
_Avoid_: Switcher keybinding, internal shortcut

**Switcher Preferences**:
The app-owned visual and performance settings persisted through a versioned `cosmic-config` schema: card size, background dimming, thumbnail refresh limit, animations, and reveal delay. They do not duplicate COSMIC Workspace Policy, COSMIC Accessibility Policy, or COSMIC Shortcut Policy.
_Avoid_: System settings, copied COSMIC settings

**Session Preferences**:
The immutable snapshot of Switcher Preferences taken when a Switching Session begins. Preference edits persist immediately but affect only later sessions, preventing an active Switcher Grid from changing beneath the user's selection.
_Avoid_: Live settings, delayed save

**Visible Workspace Set**:
The workspace spanning all displays, or the independently active workspace on each display, as determined by COSMIC Workspace Policy.
_Avoid_: Current workspace

**Window Scope**:
The rule selecting which discovered Windows enter a Session Window Set. All Workspaces is the default; Visible Workspaces is the capability-gated stricter scope.
_Avoid_: COSMIC Workspace Policy, guessed workspace membership

**All Workspaces**:
The Window Scope in which every independently exposed application Window participates in one global MRU Order, including Windows on inactive workspaces.
_Avoid_: Unfiltered fallback, guessed visibility

**Visible Workspaces**:
The Window Scope in which only Windows belonging to the Visible Workspace Set participate. It requires an authoritative committed membership snapshot from COSMIC.
_Avoid_: Current workspace only, inferred workspace

**Eligible Window**:
A Native Wayland Window or XWayland Window admitted by the active Window Scope, including a minimized Window. Shell surfaces remain excluded in every scope.
_Avoid_: App, focused-display window

**Minimized Window**:
An Eligible Window hidden through minimization. Selecting it restores and focuses it; its Switcher Item uses the last available thumbnail or falls back to its application icon and title.
_Avoid_: Hidden window

**Live Thumbnail**:
A damage-driven, uncropped visual representation of a Window's current full contents while its row intersects the Grid Viewport. Changed content refreshes up to the Refresh Ceiling; the compositor need not emit duplicate frames for unchanged content. A stale frame or icon-and-title fallback is degraded operation, not a substitute for live capture.
_Avoid_: Preview, screenshot, static thumbnail

**Capture Backend**:
The automatically selected in-memory transport for Live Thumbnail frames. The Switcher Service prefers GPU-native DMA-BUF and falls back to CPU-backed shared memory when DMA-BUF is unavailable or cannot be imported; this is diagnostic state, not a Switcher Preference.
_Avoid_: Capture quality setting, persistent frame store

**Refresh Ceiling**:
The user-selected maximum Live Thumbnail frame rate, not a request for duplicate unchanged frames or a guaranteed rate under overload. Input is always scheduled first, followed by the selected Switcher Item and then fair round-robin updates for every other visible item; effective rates recover automatically without reducing thumbnail resolution.
_Avoid_: Guaranteed frame rate, thumbnail quality

**Switcher Grid**:
The centered adaptive arrangement of every Switcher Item in one continuous MRU layout. Up to five Switcher Items occupy a row: one through five Windows use one row, six use two rows of three, and larger sets use rows of five. Card geometry scales to retain two complete rows and a half-row continuation cue when more rows exist. Visual rows and scrolling never alter MRU Order.
_Avoid_: Window list, thumbnail strip, paginated grid

**Grid Viewport**:
The visible portion of the continuous Switcher Grid. Keyboard navigation automatically reveals the selected row; overflow peeks forward or backward by half a row so earlier and later content remain discoverable. Only rows intersecting the viewport receive continuous Live Thumbnail frames; an off-screen row remains part of the same grid and resumes capture when revealed.
_Avoid_: Grid page

**Switching Session**:
The interaction beginning when the switching shortcut opens the COSMIC Window Switcher and ending when a selected Window is activated or the interaction is cancelled. Cancelling with Escape preserves the Window that had focus when the session began.
_Avoid_: App switching session

**Session Deactivation**:
A screen lock, suspend, user switch, or COSMIC session shutdown. It cancels any active Switching Session without activation, destroys the overlay and capture buffers, and pauses MRU tracking until the unlocked COSMIC Session becomes active again.
_Avoid_: Session pause, resumable overlay

**Hold Mode**:
A Switching Session opened while Alt, Ctrl, or Super is held. Tab requests continue cycling, and releasing the last initially held non-Shift modifier activates the selected Window. Shift affects direction but never keeps the session open.
_Avoid_: Alt-only mode

**Latch Mode**:
A Switching Session opened without Alt, Ctrl, or Super held. It remains open until Enter or a mouse click activates a Window, or Escape cancels it.
_Avoid_: Broken modifier mode, timeout mode

**Session Window Set**:
The stable snapshot of Eligible Windows in MRU Order captured when a Switching Session begins. Closed Windows are removed, while newly opened Windows wait for the next session.
_Avoid_: Live window list, dynamic order

**Session Display**:
The display hosting the session's sole Switcher Grid. It is the initially focused Window's display when authoritative membership is available, otherwise a deterministic workspace-group output in All Workspaces.
_Avoid_: Duplicated overlay, random display

**Workspace Move**:
The v1 direct-manipulation action that drags a Switcher Item onto a COSMIC workspace target. Moving a Window outside the Visible Workspace Set removes it from the current Session Window Set without ending the Switching Session.
_Avoid_: Workspace picker, move menu
