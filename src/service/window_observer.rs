// SPDX-License-Identifier: GPL-3.0-only

use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use cosmic_client_toolkit::{
    GlobalData,
    toplevel_management::{ToplevelManagerHandler, ToplevelManagerState},
    workspace::{WorkspaceHandler, WorkspaceState},
};
use cosmic_protocols::toplevel_info::v1::client::{
    zcosmic_toplevel_handle_v1, zcosmic_toplevel_info_v1,
};
use cosmic_protocols::toplevel_management::v1::client::zcosmic_toplevel_manager_v1;
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    delegate_compositor, delegate_keyboard, delegate_layer, delegate_output, delegate_pointer,
    delegate_registry, delegate_seat, delegate_shm,
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{
        Capability, SeatHandler, SeatState,
        keyboard::{KeyEvent, KeyboardHandler, Keysym, Modifiers, RawModifiers},
        pointer::{PointerEvent, PointerEventKind, PointerHandler},
    },
    shell::{
        WaylandSurface,
        wlr_layer::{
            Anchor, KeyboardInteractivity, Layer, LayerShell, LayerShellHandler, LayerSurface,
            LayerSurfaceConfigure,
        },
    },
    shm::{Shm, ShmHandler, raw::RawPool},
};
use wayland_client::{
    Connection, Dispatch, EventQueue, Proxy, QueueHandle, WEnum, delegate_noop,
    globals::{GlobalList, registry_queue_init},
    protocol::{wl_buffer, wl_keyboard, wl_output, wl_pointer, wl_seat, wl_shm, wl_surface},
};
use wayland_protocols::ext::foreign_toplevel_list::v1::client::{
    ext_foreign_toplevel_handle_v1, ext_foreign_toplevel_list_v1,
};
use wayland_protocols::ext::{
    image_capture_source::v1::client::{
        ext_foreign_toplevel_image_capture_source_manager_v1, ext_image_capture_source_v1,
    },
    image_copy_capture::v1::client::ext_image_copy_capture_manager_v1,
    workspace::v1::client::ext_workspace_handle_v1,
};
use wayland_protocols::wp::{
    fractional_scale::v1::client::{wp_fractional_scale_manager_v1, wp_fractional_scale_v1},
    viewporter::client::{wp_viewport, wp_viewporter},
};

use cosmic_window_switcher::{
    APPLICATION_ID, AccessibilityPolicy, CaptureEffect, CaptureFailure, CaptureSessionModel,
    DesktopSnapshot, FractionalScale, GridLayout, HoldModifiers, InvocationDirection,
    InvocationRequest, Locale, OverlayPresentation, PreferencesStore, RefreshCeiling,
    ServiceEffect, ServiceEvent, SessionDisplay, SessionPreferences, ShmConstraints,
    ShmFrameLayout, SwitcherGrid, SwitcherItem, SwitchingEvent, WindowEvent, WindowId, WindowScope,
    WindowSnapshot, WorkspaceEligibilityState, WorkspaceGroupSnapshot, WorkspaceId,
    WorkspaceSnapshot,
};

use super::{
    PendingInvocations, SharedService,
    accessibility::AccessibilityBridge,
    invocation,
    overlay::{OverlayDimensions, OverlayRenderer, RenderedOverlay},
};
use crate::shm_capture::{
    CaptureFrame, CaptureFrameData, CaptureSession, CaptureSessionData, ShmCaptureHandler,
    ShmCaptureState, duration_to_timespec,
};

const SESSION_READINESS_TIMEOUT: Duration = Duration::from_millis(500);
const TOPLEVEL_INFO_INTERFACE: &str = "zcosmic_toplevel_info_v1";
const WORKSPACE_MANAGER_INTERFACE: &str = "ext_workspace_manager_v1";
const REQUIRED_TOPLEVEL_INFO_VERSION: u32 = 3;
const REQUIRED_WORKSPACE_MANAGER_VERSION: u32 = 1;

fn advertised_global_version(globals: &GlobalList, interface: &str) -> Option<u32> {
    globals.contents().with_list(|list| {
        list.iter()
            .find(|global| global.interface == interface)
            .map(|global| global.version)
    })
}

fn advertised_cosmic_versions(globals: &GlobalList) -> (Option<u32>, Option<u32>) {
    (
        advertised_global_version(globals, TOPLEVEL_INFO_INTERFACE),
        advertised_global_version(globals, WORKSPACE_MANAGER_INTERFACE),
    )
}

fn cosmic_accessibility_policy() -> AccessibilityPolicy {
    let high_contrast = cosmic::theme::system_preference()
        .theme_type
        .is_high_contrast();
    let reduced_motion = cosmic_config::Config::new("com.system76.CosmicTk", 1)
        .ok()
        .and_then(|config| cosmic_config::ConfigGet::get(&config, "reduced_motion").ok())
        .unwrap_or(false);
    AccessibilityPolicy::new(false, high_contrast, reduced_motion)
}

struct PreferenceState {
    store: PreferencesStore,
    session: SessionPreferences,
    presentation: OverlayPresentation,
}

impl PreferenceState {
    fn open() -> Result<Self> {
        let store = PreferencesStore::open().context("open app-owned Switcher Preferences")?;
        let session = store.load().snapshot();
        let presentation = OverlayPresentation::resolve(&session, cosmic_accessibility_policy());
        Ok(Self {
            store,
            session,
            presentation,
        })
    }

    fn snapshot(&mut self) {
        self.session = self.store.load().snapshot();
        self.presentation =
            OverlayPresentation::resolve(&self.session, cosmic_accessibility_policy());
    }
}

pub(super) struct WindowObserver {
    connection: Connection,
    event_queue: EventQueue<ProtocolObserver>,
    state: ProtocolObserver,
    pending_invocations: PendingInvocations,
}

impl WindowObserver {
    pub(super) fn connect(
        service: SharedService,
        pending_invocations: PendingInvocations,
    ) -> Result<Self> {
        let connection =
            Connection::connect_to_env().context("connect to the Wayland compositor")?;
        let (globals, event_queue) =
            registry_queue_init(&connection).context("read Wayland globals")?;
        let queue_handle = event_queue.handle();
        let (advertised_toplevel_info_version, advertised_workspace_manager_version) =
            advertised_cosmic_versions(&globals);
        let compositor =
            CompositorState::bind(&globals, &queue_handle).context("bind wl_compositor")?;
        let layer_shell =
            LayerShell::bind(&globals, &queue_handle).context("bind wlr layer shell")?;
        let shm = Shm::bind(&globals, &queue_handle).context("bind wl_shm")?;
        let registry_state = RegistryState::new(&globals);
        let fractional_scale_manager = registry_state
            .bind_one::<wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1, _, _>(
                &queue_handle,
                1..=1,
                (),
            )
            .ok();
        let viewporter = registry_state
            .bind_one::<wp_viewporter::WpViewporter, _, _>(&queue_handle, 1..=1, ())
            .ok();
        let capture_backend = ShmCaptureState::new(&globals, &queue_handle);
        let foreign_toplevel_list = registry_state
            .bind_one::<ext_foreign_toplevel_list_v1::ExtForeignToplevelListV1, _, _>(
                &queue_handle,
                1..=1,
                GlobalData,
            )
            .context("the compositor does not expose foreign toplevel discovery")?;
        let cosmic_toplevel_info = registry_state
            .bind_one::<zcosmic_toplevel_info_v1::ZcosmicToplevelInfoV1, _, _>(
                &queue_handle,
                2..=3,
                GlobalData,
            )
            .ok();
        let toplevel_manager = ToplevelManagerState::try_new(&registry_state, &queue_handle)
            .context("the compositor does not expose COSMIC Window management")?;
        let workspace_state = WorkspaceState::new(&registry_state, &queue_handle);
        let preferences = PreferenceState::open()?;
        let state = ProtocolObserver {
            queue_handle: queue_handle.clone(),
            registry_state,
            compositor,
            layer_shell,
            shm,
            output_state: OutputState::new(&globals, &queue_handle),
            fractional_scale_manager,
            viewporter,
            seat_state: SeatState::new(&globals, &queue_handle),
            toplevel_manager,
            workspace_state,
            _foreign_toplevel_list: foreign_toplevel_list,
            cosmic_toplevel_info,
            capture_backend,
            capture_model: CaptureSessionModel::new(RefreshCeiling::Fps30),
            capture_clock: Instant::now(),
            windows: Vec::new(),
            observations: ObservationLedger::default(),
            next_observation_key: 0,
            service,
            management_can_activate: false,
            advertised_toplevel_info_version,
            advertised_workspace_manager_version,
            workspace_snapshot_received: false,
            toplevel_snapshot_received: false,
            accessibility: AccessibilityBridge::new(Locale::detect()),
            overlay_renderer: OverlayRenderer::new(),
            preferences,
            layer: None,
            readiness_pool: None,
            readiness_buffer: None,
            grid_pool: None,
            grid_buffer: None,
            grid_dimensions: None,
            grid_layout: None,
            fractional_scale: None,
            viewport: None,
            preferred_scale: None,
            grid: None,
            session_window_order: Vec::new(),
            session_output: None,
            interaction: InteractionState::default(),
            keyboard: None,
            pointer: None,
            seat: None,
            pending_direction: None,
            initial_hold_modifiers: None,
            reveal_at: None,
            readiness_deadline: None,
        };

        Ok(Self {
            connection,
            event_queue,
            state,
            pending_invocations,
        })
    }

    pub(super) fn synchronize_initial_windows(&mut self) -> Result<()> {
        for round in 0..8 {
            self.event_queue
                .roundtrip(&mut self.state)
                .context("receive initial COSMIC Window and workspace state")?;
            if round >= 2
                && (self.state.workspace_snapshot_received
                    || self.state.advertised_workspace_manager_version
                        < Some(REQUIRED_WORKSPACE_MANAGER_VERSION))
            {
                break;
            }
        }
        let workspace_eligibility = self.state.workspace_eligibility_state();
        let mut service = self
            .state
            .service
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        service.set_workspace_eligibility_state(workspace_eligibility);
        service.complete_initial_discovery();
        Ok(())
    }

    pub(super) fn dispatch(&mut self) -> Result<()> {
        let queue_handle = self.event_queue.handle();
        self.state
            .start_pending_invocation(&self.pending_invocations, &queue_handle);
        self.state.handle_reveal_deadline();
        self.event_queue
            .dispatch_pending(&mut self.state)
            .context("dispatch pending COSMIC Window events")?;
        self.state.handle_capture_deadline();
        self.connection
            .flush()
            .context("flush the COSMIC compositor connection")?;

        let Some(read_guard) = self.event_queue.prepare_read() else {
            return Ok(());
        };
        let fd = read_guard.connection_fd();
        let invocation_fd = self.pending_invocations.wake_fd();
        let mut poll_fds = [
            rustix::event::PollFd::new(
                &fd,
                rustix::event::PollFlags::IN | rustix::event::PollFlags::ERR,
            ),
            rustix::event::PollFd::new(
                &invocation_fd,
                rustix::event::PollFlags::IN | rustix::event::PollFlags::ERR,
            ),
        ];
        let timeout = self.state.poll_timeout().map(duration_to_timespec);
        rustix::event::poll(&mut poll_fds, timeout.as_ref()).context("poll COSMIC events")?;
        if poll_fds[0]
            .revents()
            .intersects(rustix::event::PollFlags::IN | rustix::event::PollFlags::ERR)
        {
            read_guard.read().context("read COSMIC Window events")?;
        } else {
            drop(read_guard);
        }
        self.event_queue
            .dispatch_pending(&mut self.state)
            .context("observe COSMIC Window events")?;
        self.state
            .start_pending_invocation(&self.pending_invocations, &queue_handle);
        self.state.handle_reveal_deadline();
        self.state.handle_capture_deadline();
        Ok(())
    }
}

#[derive(Clone)]
struct ObservedWindow {
    key: ObservationKey,
    foreign_toplevel: ext_foreign_toplevel_handle_v1::ExtForeignToplevelHandleV1,
    cosmic_toplevel: Option<zcosmic_toplevel_handle_v1::ZcosmicToplevelHandleV1>,
    title: String,
    application_id: String,
    outputs: Vec<wl_output::WlOutput>,
    geometries: std::collections::HashMap<wl_output::WlOutput, WindowGeometry>,
    workspaces: std::collections::HashSet<ext_workspace_handle_v1::ExtWorkspaceHandleV1>,
    committed_workspaces: std::collections::HashSet<ext_workspace_handle_v1::ExtWorkspaceHandleV1>,
    minimized: bool,
    fullscreen: bool,
    sticky: bool,
}

#[derive(Clone, Copy)]
struct WindowGeometry {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}

#[derive(Default)]
struct InteractionState {
    keyboard_focused: bool,
    shift_active: bool,
    visible: bool,
}

struct ProtocolObserver {
    queue_handle: QueueHandle<Self>,
    registry_state: RegistryState,
    compositor: CompositorState,
    layer_shell: LayerShell,
    shm: Shm,
    output_state: OutputState,
    fractional_scale_manager: Option<wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1>,
    viewporter: Option<wp_viewporter::WpViewporter>,
    seat_state: SeatState,
    toplevel_manager: ToplevelManagerState,
    workspace_state: WorkspaceState,
    capture_backend: ShmCaptureState,
    capture_model: CaptureSessionModel,
    capture_clock: Instant,
    _foreign_toplevel_list: ext_foreign_toplevel_list_v1::ExtForeignToplevelListV1,
    cosmic_toplevel_info: Option<zcosmic_toplevel_info_v1::ZcosmicToplevelInfoV1>,
    windows: Vec<ObservedWindow>,
    observations: ObservationLedger,
    next_observation_key: u64,
    service: SharedService,
    management_can_activate: bool,
    advertised_toplevel_info_version: Option<u32>,
    advertised_workspace_manager_version: Option<u32>,
    workspace_snapshot_received: bool,
    toplevel_snapshot_received: bool,
    accessibility: AccessibilityBridge,
    overlay_renderer: OverlayRenderer,
    preferences: PreferenceState,
    layer: Option<LayerSurface>,
    readiness_pool: Option<RawPool>,
    readiness_buffer: Option<wl_buffer::WlBuffer>,
    grid_pool: Option<RawPool>,
    grid_buffer: Option<wl_buffer::WlBuffer>,
    grid_dimensions: Option<OverlayDimensions>,
    grid_layout: Option<GridLayout>,
    fractional_scale: Option<wp_fractional_scale_v1::WpFractionalScaleV1>,
    viewport: Option<wp_viewport::WpViewport>,
    preferred_scale: Option<FractionalScale>,
    grid: Option<SwitcherGrid>,
    session_window_order: Vec<WindowId>,
    session_output: Option<wl_output::WlOutput>,
    interaction: InteractionState,
    keyboard: Option<wl_keyboard::WlKeyboard>,
    pointer: Option<wl_pointer::WlPointer>,
    seat: Option<wl_seat::WlSeat>,
    pending_direction: Option<InvocationDirection>,
    initial_hold_modifiers: Option<HoldModifiers>,
    reveal_at: Option<Instant>,
    readiness_deadline: Option<Instant>,
}

impl ProtocolObserver {
    fn apply(&mut self, event: Observation) {
        let closed = match &event {
            Observation::Closed(key) => self.observations.window_id(*key).cloned(),
            _ => None,
        };
        let window_events = self.observations.apply(event);
        if let Some(closed) = closed.as_ref() {
            self.session_window_order.retain(|window| window != closed);
            let effects = self.capture_model.window_closed(closed);
            if let Err(error) = self.apply_capture_effects(effects) {
                self.fail_overlay("release closed Window capture failed", &error);
            }
        }
        let grid_changed = closed
            .as_ref()
            .is_some_and(|closed| self.grid.as_mut().is_some_and(|grid| grid.remove(closed)));
        let mut service = self
            .service
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for window_event in window_events {
            let effects = service.observe(window_event);
            drop(service);
            self.apply_effects(effects);
            service = self
                .service
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
        }
        drop(service);
        if grid_changed && self.grid.is_some() {
            if let Err(error) = self.render_grid() {
                self.fail_overlay("render Switcher Grid after Window closure failed", &error);
            } else if self.interaction.visible {
                self.accessibility
                    .update(self.grid.as_ref().expect("the Switcher Grid is present"));
            }
        }
    }

    fn start_pending_invocation(
        &mut self,
        pending_invocations: &PendingInvocations,
        queue_handle: &QueueHandle<Self>,
    ) {
        let direction = pending_invocations.pop();
        let Some(direction) = direction else {
            return;
        };
        if self.layer.is_some() {
            let effects = self
                .service
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .handle(ServiceEvent::Invocation(direction));
            self.apply_effects(effects);
            return;
        }

        let diagnostics = self
            .service
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .diagnostics();
        let complete_mru_order = diagnostics.mru_order;
        let window_scope = diagnostics.window_scope;
        if complete_mru_order.len() < 2 {
            return;
        }
        if !self.management_can_activate || self.seat.is_none() {
            self.fallback(direction);
            return;
        }
        let workspace_fallback = self
            .service
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .workspace_invocation_fallback(direction);
        if let Some(effect) = workspace_fallback {
            self.apply_effects(vec![effect]);
            return;
        }
        let Some(context) = self
            .desktop_snapshot()
            .switching_context(window_scope, complete_mru_order.clone())
        else {
            self.fallback(direction);
            return;
        };
        if context.eligible_windows.len() < 2 {
            return;
        }
        let Some(session_output) = complete_mru_order
            .first()
            .and_then(|focused| self.observed_window(focused))
            .and_then(|window| {
                window.outputs.iter().find(|output| {
                    self.output_display(output)
                        .is_some_and(|display| display == context.session_display)
                })
            })
            .cloned()
            .or_else(|| {
                self.output_state.outputs().find(|output| {
                    self.output_display(output)
                        .is_some_and(|display| display == context.session_display)
                })
            })
        else {
            self.fallback(direction);
            return;
        };

        self.snapshot_session_preferences(&session_output);

        let surface = self.compositor.create_surface(queue_handle);
        let layer = self.layer_shell.create_layer_surface(
            queue_handle,
            surface,
            Layer::Overlay,
            Some(APPLICATION_ID),
            Some(&session_output),
        );
        self.fractional_scale = self
            .fractional_scale_manager
            .as_ref()
            .map(|manager| manager.get_fractional_scale(layer.wl_surface(), queue_handle, ()));
        self.viewport = self
            .viewporter
            .as_ref()
            .map(|viewporter| viewporter.get_viewport(layer.wl_surface(), queue_handle, ()));
        self.preferred_scale = None;
        layer.set_anchor(Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT);
        layer.set_keyboard_interactivity(KeyboardInteractivity::Exclusive);
        layer.set_size(0, 0);
        layer.commit();
        self.layer = Some(layer);
        self.session_window_order = context.eligible_windows;
        self.session_output = Some(session_output);
        self.grid = None;
        self.interaction = InteractionState::default();
        self.pending_direction = Some(direction);
        self.initial_hold_modifiers = None;
        let now = Instant::now();
        self.reveal_at = Some(now + self.preferences.session.reveal_delay().duration());
        self.readiness_deadline = Some(now + SESSION_READINESS_TIMEOUT);
    }

    fn snapshot_session_preferences(&mut self, session_output: &wl_output::WlOutput) {
        self.preferences.snapshot();
        self.accessibility.set_locale(Locale::detect());
        self.capture_model = CaptureSessionModel::new(self.preferences.session.refresh_ceiling());
        if self.preferences.session.refresh_ceiling() == RefreshCeiling::MatchDisplay
            && let Some(refresh_rate) = self
                .output_state
                .info(session_output)
                .and_then(|info| {
                    info.modes
                        .into_iter()
                        .find(|mode| mode.current && mode.refresh_rate > 0)
                })
                .and_then(|mode| u16::try_from((mode.refresh_rate + 999) / 1_000).ok())
        {
            self.capture_model.set_display_refresh_rate(refresh_rate);
        }
    }

    fn poll_timeout(&self) -> Option<Duration> {
        let overlay_timeout = [self.reveal_at, self.readiness_deadline]
            .into_iter()
            .flatten()
            .min()
            .map(|deadline| deadline.saturating_duration_since(Instant::now()));
        let capture_timeout = self
            .capture_model
            .next_request_at()
            .map(|deadline| deadline.saturating_sub(self.capture_clock.elapsed()));
        [overlay_timeout, capture_timeout]
            .into_iter()
            .flatten()
            .min()
    }

    fn handle_capture_deadline(&mut self) {
        let effects = self.capture_model.refresh_due(self.capture_clock.elapsed());
        if let Err(error) = self.apply_capture_effects(effects) {
            self.fail_overlay("request refreshed Live Thumbnail failed", &error);
        }
    }

    fn try_mark_ready(&mut self) {
        if !self.interaction.keyboard_focused
            || self.grid_pool.is_none()
            || self.grid_buffer.is_none()
            || self.initial_hold_modifiers.is_none()
        {
            return;
        }
        let effects = self
            .service
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .handle(ServiceEvent::SessionReady);
        self.readiness_deadline = None;
        self.apply_effects(effects);
    }

    fn handle_reveal_deadline(&mut self) {
        if self
            .readiness_deadline
            .is_some_and(|deadline| Instant::now() >= deadline)
        {
            self.readiness_deadline = None;
            let direction = self.pending_direction;
            let effects = self
                .service
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .handle(ServiceEvent::SessionReadinessFailed);
            if effects.is_empty() {
                if let Some(direction) = direction {
                    self.fallback(direction);
                }
            } else {
                self.apply_effects(effects);
            }
            return;
        }
        if self
            .reveal_at
            .is_some_and(|deadline| Instant::now() >= deadline)
        {
            self.reveal_at = None;
            let effects = self
                .service
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .handle(ServiceEvent::RevealDelayElapsed);
            self.apply_effects(effects);
        }
    }

    fn handle_switching_event(&mut self, event: SwitchingEvent) {
        let service_event = match event {
            SwitchingEvent::HoldModifiersChanged(modifiers) => {
                ServiceEvent::HoldModifiersChanged(modifiers)
            }
            SwitchingEvent::Tab
            | SwitchingEvent::Navigate(_)
            | SwitchingEvent::Enter
            | SwitchingEvent::Escape => ServiceEvent::Switching(event),
        };
        self.handle_service_event(service_event);
    }

    fn window_at_pointer(&self, position: (f64, f64)) -> Option<WindowId> {
        self.grid
            .as_ref()?
            .window_at(self.grid_layout.as_ref()?, position.0, position.1)
            .cloned()
    }

    fn handle_service_event(&mut self, event: ServiceEvent) {
        let effects = self
            .service
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .handle(event);
        self.apply_effects(effects);
    }

    fn apply_effects(&mut self, effects: Vec<ServiceEffect>) {
        for effect in effects {
            match effect {
                ServiceEffect::PrepareInvisibleOverlay { selected } => {
                    if let Err(error) = self.prepare_grid(&selected) {
                        self.fail_overlay("prepare Switcher Grid failed", &error);
                    }
                }
                ServiceEffect::SelectionChanged(selected) => {
                    if let Err(error) = self.select_grid(&selected) {
                        self.fail_overlay("render Switcher Grid selection failed", &error);
                    }
                }
                ServiceEffect::RevealOverlay { selected } => {
                    if let Some(grid) = self.grid.as_mut() {
                        let _selection = grid.select(&selected);
                    }
                    if let Err(error) = self.show_grid() {
                        self.fail_overlay("reveal Switcher Grid failed", &error);
                    }
                }
                ServiceEffect::Activate(window) => {
                    let target = self
                        .windows
                        .iter()
                        .find(|candidate| {
                            self.observations.window_id(candidate.key) == Some(&window)
                        })
                        .and_then(|candidate| candidate.cosmic_toplevel.clone());
                    if let (Some(target), Some(seat)) = (target, self.seat.as_ref()) {
                        self.toplevel_manager.manager.activate(&target, seat);
                    }
                    self.destroy_overlay();
                }
                ServiceEffect::Cancel => self.destroy_overlay(),
                ServiceEffect::FallbackToStockSwitcher(direction) => self.fallback(direction),
            }
        }
    }

    fn fallback(&mut self, direction: InvocationDirection) {
        self.destroy_overlay();
        if let Err(error) = invocation::launch_stock_switcher(direction) {
            eprintln!("stock COSMIC switcher fallback failed: {error:#}");
        }
    }

    fn fail_overlay(&mut self, context: &str, error: &anyhow::Error) {
        eprintln!("{context}: {error:#}");
        if let Some(direction) = self.pending_direction {
            self.fallback(direction);
        }
    }

    fn destroy_overlay(&mut self) {
        let capture_effects = self.capture_model.stop();
        if let Err(error) = self.apply_capture_effects(capture_effects) {
            eprintln!("release Live Thumbnail capture failed: {error:#}");
        }
        self.capture_backend.stop_all();
        if let Some(fractional_scale) = self.fractional_scale.take() {
            fractional_scale.destroy();
        }
        if let Some(viewport) = self.viewport.take() {
            viewport.destroy();
        }
        if let Some(layer) = self.layer.take() {
            layer.set_keyboard_interactivity(KeyboardInteractivity::None);
            layer.commit();
        }
        if let Some(buffer) = self.grid_buffer.take() {
            buffer.destroy();
        }
        if let Some(buffer) = self.readiness_buffer.take() {
            buffer.destroy();
        }
        self.readiness_pool = None;
        self.grid_pool = None;
        self.grid_dimensions = None;
        self.grid_layout = None;
        self.preferred_scale = None;
        self.grid = None;
        self.accessibility.hide();
        self.session_window_order.clear();
        self.session_output = None;
        self.interaction = InteractionState::default();
        self.pending_direction = None;
        self.initial_hold_modifiers = None;
        self.reveal_at = None;
        self.readiness_deadline = None;
    }

    fn observed_window(&self, id: &WindowId) -> Option<&ObservedWindow> {
        self.windows
            .iter()
            .find(|window| self.observations.window_id(window.key) == Some(id))
    }

    fn commit_toplevel_snapshot(&mut self) {
        for window in &mut self.windows {
            window.committed_workspaces.clone_from(&window.workspaces);
        }
        self.toplevel_snapshot_received = self
            .windows
            .iter()
            .all(|window| window.sticky || !window.committed_workspaces.is_empty());
        self.update_workspace_eligibility_state();
    }

    fn workspace_eligibility_state(&self) -> WorkspaceEligibilityState {
        let Some(toplevel_version) = self
            .advertised_toplevel_info_version
            .filter(|version| *version >= REQUIRED_TOPLEVEL_INFO_VERSION)
        else {
            return WorkspaceEligibilityState::MissingToplevelInfo {
                advertised_version: self.advertised_toplevel_info_version,
                required_version: REQUIRED_TOPLEVEL_INFO_VERSION,
            };
        };
        let Some(workspace_version) = self
            .advertised_workspace_manager_version
            .filter(|version| *version >= REQUIRED_WORKSPACE_MANAGER_VERSION)
        else {
            return WorkspaceEligibilityState::MissingWorkspaceProtocol {
                advertised_version: self.advertised_workspace_manager_version,
                required_version: REQUIRED_WORKSPACE_MANAGER_VERSION,
            };
        };
        if !self.workspace_snapshot_received {
            return WorkspaceEligibilityState::MissingWorkspaceSnapshot {
                advertised_version: workspace_version,
            };
        }
        if !self.toplevel_snapshot_received {
            return WorkspaceEligibilityState::MissingToplevelMembership {
                advertised_version: toplevel_version,
            };
        }
        WorkspaceEligibilityState::Ready
    }

    fn update_workspace_eligibility_state(&self) {
        let state = self.workspace_eligibility_state();
        self.service
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .set_workspace_eligibility_state(state);
    }

    fn await_toplevel_snapshot(&mut self) {
        self.toplevel_snapshot_received = false;
        let workspace_filtering_required = self
            .service
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .window_scope()
            == WindowScope::VisibleWorkspaces;
        if workspace_filtering_required
            && self
                .advertised_toplevel_info_version
                .is_some_and(|version| version >= REQUIRED_TOPLEVEL_INFO_VERSION)
            && self
                .advertised_workspace_manager_version
                .is_some_and(|version| version >= REQUIRED_WORKSPACE_MANAGER_VERSION)
            && self.workspace_snapshot_received
        {
            self.service
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .set_workspace_eligibility_state(WorkspaceEligibilityState::AwaitingSnapshot);
        } else {
            self.update_workspace_eligibility_state();
        }
    }

    fn desktop_snapshot(&self) -> DesktopSnapshot {
        let workspace_groups = self
            .workspace_state
            .workspace_groups()
            .map(|group| WorkspaceGroupSnapshot {
                outputs: group
                    .outputs
                    .iter()
                    .filter_map(|output| self.output_display(output))
                    .collect(),
                workspaces: group
                    .workspaces
                    .iter()
                    .map(|workspace| self.workspace_id(workspace))
                    .collect(),
            })
            .collect();
        let workspaces = self
            .workspace_state
            .workspaces()
            .map(|workspace| WorkspaceSnapshot {
                id: self.workspace_id(&workspace.handle),
                active: workspace
                    .state
                    .contains(ext_workspace_handle_v1::State::Active),
                hidden: workspace
                    .state
                    .contains(ext_workspace_handle_v1::State::Hidden),
            })
            .collect();
        let windows = self
            .windows
            .iter()
            .filter_map(|window| {
                Some(WindowSnapshot {
                    id: self.observations.window_id(window.key)?.clone(),
                    workspace_membership: window
                        .committed_workspaces
                        .iter()
                        .map(|workspace| self.workspace_id(workspace))
                        .collect(),
                    output_membership: window
                        .outputs
                        .iter()
                        .filter_map(|output| self.output_display(output))
                        .collect(),
                    session_display: self
                        .session_output_for_window(window)
                        .and_then(|output| self.output_display(output)),
                    minimized: window.minimized,
                    fullscreen: window.fullscreen,
                    sticky: window.sticky,
                })
            })
            .collect();

        DesktopSnapshot {
            workspace_groups,
            workspaces,
            windows,
        }
    }

    fn output_display(&self, output: &wl_output::WlOutput) -> Option<SessionDisplay> {
        self.output_state.info(output).map(|info| {
            SessionDisplay::from(info.name.unwrap_or_else(|| format!("output-{}", info.id)))
        })
    }

    fn session_output_for_window<'a>(
        &self,
        window: &'a ObservedWindow,
    ) -> Option<&'a wl_output::WlOutput> {
        if let [only] = window.outputs.as_slice() {
            return Some(only);
        }
        window
            .outputs
            .iter()
            .filter(|output| window.geometries.contains_key(*output))
            .max_by(|left, right| {
                let left_area = self.window_area_on_output(window, left);
                let right_area = self.window_area_on_output(window, right);
                left_area.cmp(&right_area).then_with(|| {
                    let left_name = self.output_display(left).map(|display| display.to_string());
                    let right_name = self
                        .output_display(right)
                        .map(|display| display.to_string());
                    // Reverse the lexical tie-break so `max_by` chooses the
                    // stable lowest output name when visible areas are equal.
                    right_name.cmp(&left_name)
                })
            })
    }

    fn window_area_on_output(&self, window: &ObservedWindow, output: &wl_output::WlOutput) -> u64 {
        let Some(geometry) = window.geometries.get(output) else {
            return 0;
        };
        let Some((output_width, output_height)) = self
            .output_state
            .info(output)
            .and_then(|info| info.logical_size)
        else {
            return 0;
        };
        let left = geometry.x.max(0);
        let top = geometry.y.max(0);
        let right = geometry
            .x
            .saturating_add(geometry.width.max(0))
            .min(output_width);
        let bottom = geometry
            .y
            .saturating_add(geometry.height.max(0))
            .min(output_height);
        let width = u64::try_from(right.saturating_sub(left)).unwrap_or(0);
        let height = u64::try_from(bottom.saturating_sub(top)).unwrap_or(0);
        width.saturating_mul(height)
    }

    fn workspace_id(&self, handle: &ext_workspace_handle_v1::ExtWorkspaceHandleV1) -> WorkspaceId {
        self.workspace_state
            .workspace_info(handle)
            .and_then(|workspace| workspace.id.as_deref())
            .map_or_else(
                || WorkspaceId::from(format!("wayland:{}", handle.id().protocol_id())),
                |id| WorkspaceId::from(format!("id:{id}")),
            )
    }

    fn apply_capture_effects(&mut self, effects: Vec<CaptureEffect>) -> Result<()> {
        for effect in effects {
            match effect {
                CaptureEffect::CreateStream(window) => {
                    if let Err(error) = self.create_capture_stream(&window) {
                        self.degrade_capture_after_error(&window, &error)?;
                    }
                }
                CaptureEffect::RequestFrame { window, layout } => {
                    if let Err(error) = self.request_capture_frame(&window, layout) {
                        self.degrade_capture_after_error(&window, &error)?;
                    }
                }
                CaptureEffect::PresentThumbnail(_) => {}
                CaptureEffect::DegradeThumbnail { window, reason } => {
                    self.release_capture_stream(&window);
                    let changed = self
                        .grid
                        .as_mut()
                        .is_some_and(|grid| grid.degrade_thumbnail(&window, reason));
                    if changed {
                        self.render_grid()?;
                    }
                }
                CaptureEffect::ReleaseStream(window) => self.release_capture_stream(&window),
            }
        }
        Ok(())
    }

    fn degrade_capture_after_error(
        &mut self,
        window: &WindowId,
        error: &anyhow::Error,
    ) -> Result<()> {
        eprintln!("Live Thumbnail capture for Window {window} degraded: {error:#}");
        let effects = self
            .capture_model
            .failed(window, CaptureFailure::FrameFailed);
        self.apply_capture_effects(effects)
    }

    fn create_capture_stream(&mut self, window: &WindowId) -> Result<()> {
        let source = self
            .observed_window(window)
            .map(|window| window.foreign_toplevel.clone())
            .context("a visible Window has no capture source")?;
        self.capture_backend
            .create_stream(window.clone(), &source, &self.queue_handle)
            .context("create shared-memory capture session")
    }

    fn request_capture_frame(&mut self, window: &WindowId, layout: ShmFrameLayout) -> Result<()> {
        self.capture_backend
            .request_frame(window, layout, &self.shm, &self.queue_handle)
    }

    fn release_capture_stream(&mut self, window: &WindowId) {
        self.capture_backend.stop_stream(window);
    }

    fn prepare_grid(&mut self, selected: &WindowId) -> Result<()> {
        let output = self
            .session_output
            .as_ref()
            .context("the Switching Session has no Session Display")?;
        let output_info = self
            .output_state
            .info(output)
            .context("the Session Display has no output information")?;
        let session_display = SessionDisplay::from(
            output_info
                .name
                .unwrap_or_else(|| format!("output-{}", output_info.id)),
        );
        let items = self
            .session_window_order
            .iter()
            .map(|id| {
                self.observed_window(id).map(|window| {
                    SwitcherItem::new(
                        id.clone(),
                        window.application_id.clone(),
                        window.title.clone(),
                    )
                })
            })
            .collect::<Option<Vec<_>>>()
            .context("a Session Window is missing icon-and-title metadata")?;
        self.grid = Some(
            SwitcherGrid::new(session_display, items, selected)
                .context("the Initial Selection is absent from the Switcher Grid")?,
        );
        self.render_grid()
    }

    fn select_grid(&mut self, selected: &WindowId) -> Result<()> {
        self.grid
            .as_mut()
            .context("the Switching Session has no Switcher Grid")?
            .select(selected)
            .context("the selected Window is absent from the Switcher Grid")?;
        self.render_grid()?;
        if self.interaction.visible {
            self.accessibility
                .update(self.grid.as_ref().expect("the Switcher Grid is present"));
        }
        Ok(())
    }

    fn render_grid(&mut self) -> Result<()> {
        let output = self
            .session_output
            .as_ref()
            .context("the Switching Session has no Session Display")?;
        let output_info = self
            .output_state
            .info(output)
            .context("the Session Display has no output information")?;
        let surface_width = output_info
            .logical_size
            .and_then(|(width, _)| u32::try_from(width).ok())
            .unwrap_or(1_200);
        let surface_height = output_info
            .logical_size
            .and_then(|(_, height)| u32::try_from(height).ok())
            .unwrap_or(800);
        let output_scale = FractionalScale::from_integer(
            u32::try_from(output_info.scale_factor.max(1)).unwrap_or(1),
        );
        let scale = self
            .preferred_scale
            .filter(|_| self.viewport.is_some())
            .unwrap_or(output_scale);
        let rendered = self.overlay_renderer.render(
            self.grid
                .as_mut()
                .context("the Switching Session has no Switcher Grid")?,
            surface_width,
            surface_height,
            self.preferences.session.card_size(),
            scale,
            self.preferences.presentation,
        )?;
        self.install_rendered_grid(rendered)
    }

    fn install_rendered_grid(&mut self, rendered: RenderedOverlay) -> Result<()> {
        let RenderedOverlay {
            dimensions,
            pixels,
            layout,
        } = rendered;
        let mut pool = RawPool::new(pixels.len(), &self.shm)
            .context("allocate shared memory for the Switcher Grid")?;
        pool.mmap().copy_from_slice(&pixels);
        let (physical_width, physical_height) = dimensions.physical_size();
        let stride = physical_width
            .checked_mul(4)
            .context("Switcher Grid stride overflow")?;
        let buffer = pool.create_buffer(
            0,
            i32::try_from(physical_width).context("Switcher Grid is too wide")?,
            i32::try_from(physical_height).context("Switcher Grid is too tall")?,
            i32::try_from(stride).context("Switcher Grid stride is too large")?,
            wl_shm::Format::Argb8888,
            (),
            &self.queue_handle,
        );
        if let Some(previous) = self.grid_buffer.replace(buffer) {
            previous.destroy();
        }
        self.grid_pool = Some(pool);
        self.grid_dimensions = Some(dimensions);
        let selected = self
            .grid
            .as_ref()
            .and_then(SwitcherGrid::selected_window)
            .cloned();
        self.capture_model.set_selected(selected);
        if !self.capture_backend.contract_available() && !layout.visible_item_range().is_empty() {
            bail!("the COSMIC Session does not expose required shared-memory capture protocols");
        }
        let visible_windows = self
            .grid
            .as_ref()
            .context("the Switching Session has no Switcher Grid")?
            .visible_windows(&layout);
        let effects = self.capture_model.set_visible(visible_windows);
        self.grid_layout = Some(layout);
        self.apply_capture_effects(effects)?;
        if self.interaction.visible {
            self.attach_grid_buffer()?;
        }
        self.try_mark_ready();
        Ok(())
    }

    fn show_grid(&mut self) -> Result<()> {
        self.attach_grid_buffer()?;
        self.interaction.visible = true;
        self.accessibility
            .update(self.grid.as_ref().context("the Switcher Grid is absent")?);
        Ok(())
    }

    fn attach_grid_buffer(&self) -> Result<()> {
        let layer = self
            .layer
            .as_ref()
            .context("the Switching Session has no layer surface")?;
        let buffer = self
            .grid_buffer
            .as_ref()
            .context("the Switcher Grid has no rendered buffer")?;
        let dimensions = self
            .grid_dimensions
            .context("the Switcher Grid has no rendered dimensions")?;
        layer.set_size(dimensions.logical_width, dimensions.logical_height);
        if let Some(viewport) = self.viewport.as_ref() {
            layer.wl_surface().set_buffer_scale(1);
            viewport.set_destination(
                i32::try_from(dimensions.logical_width)
                    .context("Switcher Grid logical width is too large")?,
                i32::try_from(dimensions.logical_height)
                    .context("Switcher Grid logical height is too large")?,
            );
        } else {
            layer.wl_surface().set_buffer_scale(dimensions.buffer_scale);
        }
        layer.wl_surface().attach(Some(buffer), 0, 0);
        layer.wl_surface().damage_buffer(
            0,
            0,
            i32::try_from(dimensions.physical_size().0).context("Switcher Grid is too wide")?,
            i32::try_from(dimensions.physical_size().1).context("Switcher Grid is too tall")?,
        );
        layer.commit();
        Ok(())
    }
}

impl CompositorHandler for ProtocolObserver {
    fn scale_factor_changed(
        &mut self,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
        surface: &wl_surface::WlSurface,
        _new_factor: i32,
    ) {
        if self
            .layer
            .as_ref()
            .is_some_and(|layer| layer.wl_surface() == surface)
            && self.preferred_scale.is_none()
            && self.grid.is_some()
            && let Err(error) = self.render_grid()
        {
            self.fail_overlay("render rescaled Switcher Grid failed", &error);
        }
    }

    fn transform_changed(
        &mut self,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _new_transform: wl_output::Transform,
    ) {
    }

    fn frame(
        &mut self,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _time: u32,
    ) {
    }

    fn surface_enter(
        &mut self,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _output: &wl_output::WlOutput,
    ) {
    }

    fn surface_leave(
        &mut self,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _output: &wl_output::WlOutput,
    ) {
    }
}

impl OutputHandler for ProtocolObserver {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }

    fn new_output(
        &mut self,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }

    fn update_output(
        &mut self,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }

    fn output_destroyed(
        &mut self,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }
}

impl LayerShellHandler for ProtocolObserver {
    fn closed(
        &mut self,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
        layer: &LayerSurface,
    ) {
        if self.layer.as_ref() == Some(layer)
            && let Some(direction) = self.pending_direction
        {
            self.fallback(direction);
        }
    }

    fn configure(
        &mut self,
        _connection: &Connection,
        queue_handle: &QueueHandle<Self>,
        layer: &LayerSurface,
        _configure: LayerSurfaceConfigure,
        _serial: u32,
    ) {
        if self.readiness_pool.is_some() || self.layer.as_ref() != Some(layer) {
            return;
        }

        match RawPool::new(4, &self.shm) {
            Ok(mut pool) => {
                pool.mmap().fill(0);
                let buffer =
                    pool.create_buffer(0, 1, 1, 4, wl_shm::Format::Argb8888, (), queue_handle);
                layer.wl_surface().attach(Some(&buffer), 0, 0);
                layer.wl_surface().damage_buffer(0, 0, 1, 1);
                layer.commit();
                self.readiness_buffer = Some(buffer);
                self.readiness_pool = Some(pool);
                self.try_mark_ready();
            }
            Err(_) => {
                if let Some(direction) = self.pending_direction {
                    self.fallback(direction);
                }
            }
        }
    }
}

impl SeatHandler for ProtocolObserver {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seat_state
    }

    fn new_seat(
        &mut self,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
        _seat: wl_seat::WlSeat,
    ) {
    }

    fn new_capability(
        &mut self,
        _connection: &Connection,
        queue_handle: &QueueHandle<Self>,
        seat: wl_seat::WlSeat,
        capability: Capability,
    ) {
        if capability == Capability::Keyboard && self.keyboard.is_none() {
            match self.seat_state.get_keyboard(queue_handle, &seat, None) {
                Ok(keyboard) => {
                    self.keyboard = Some(keyboard);
                    self.seat = Some(seat);
                }
                Err(_) => {
                    if let Some(direction) = self.pending_direction {
                        self.fallback(direction);
                    }
                }
            }
        } else if capability == Capability::Pointer
            && self.pointer.is_none()
            && let Ok(pointer) = self.seat_state.get_pointer(queue_handle, &seat)
        {
            self.pointer = Some(pointer);
        }
    }

    fn remove_capability(
        &mut self,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
        _seat: wl_seat::WlSeat,
        capability: Capability,
    ) {
        if capability == Capability::Keyboard {
            if let Some(keyboard) = self.keyboard.take() {
                keyboard.release();
            }
            self.seat = None;
            if let Some(direction) = self.pending_direction {
                self.fallback(direction);
            }
        } else if capability == Capability::Pointer
            && let Some(pointer) = self.pointer.take()
        {
            pointer.release();
        }
    }

    fn remove_seat(
        &mut self,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
        _seat: wl_seat::WlSeat,
    ) {
    }
}

impl KeyboardHandler for ProtocolObserver {
    fn enter(
        &mut self,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        surface: &wl_surface::WlSurface,
        _serial: u32,
        _raw: &[u32],
        keysyms: &[Keysym],
    ) {
        if !self
            .layer
            .as_ref()
            .is_some_and(|layer| layer.wl_surface() == surface)
        {
            return;
        }
        let Some(direction) = self.pending_direction else {
            return;
        };
        let modifiers = hold_modifiers_from_keysyms(keysyms);
        self.initial_hold_modifiers = Some(modifiers);
        self.interaction.keyboard_focused = true;
        let effects = self
            .service
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .invoke_for_window_set(
                InvocationRequest {
                    direction,
                    initial_hold_modifiers: modifiers,
                },
                self.session_window_order.clone(),
            );
        let invocation_became_a_no_op = effects.is_empty() && self.session_window_order.len() < 2;
        self.apply_effects(effects);
        if invocation_became_a_no_op {
            self.destroy_overlay();
            return;
        }
        self.try_mark_ready();
    }

    fn leave(
        &mut self,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _surface: &wl_surface::WlSurface,
        _serial: u32,
    ) {
    }

    fn press_key(
        &mut self,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _serial: u32,
        event: KeyEvent,
    ) {
        match event.keysym {
            Keysym::Tab if self.interaction.shift_active => {
                self.handle_switching_event(SwitchingEvent::Navigate(
                    InvocationDirection::Previous,
                ));
            }
            Keysym::Tab => self.handle_switching_event(SwitchingEvent::Tab),
            Keysym::Return | Keysym::KP_Enter => {
                self.handle_switching_event(SwitchingEvent::Enter);
            }
            Keysym::Escape => self.handle_switching_event(SwitchingEvent::Escape),
            _ => {}
        }
    }

    fn repeat_key(
        &mut self,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _serial: u32,
        event: KeyEvent,
    ) {
        if event.keysym == Keysym::Tab {
            let direction = if self.interaction.shift_active {
                InvocationDirection::Previous
            } else {
                InvocationDirection::Next
            };
            self.handle_switching_event(SwitchingEvent::Navigate(direction));
        }
    }

    fn release_key(
        &mut self,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _serial: u32,
        _event: KeyEvent,
    ) {
    }

    fn update_modifiers(
        &mut self,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _serial: u32,
        modifiers: Modifiers,
        _raw_modifiers: RawModifiers,
        _layout: u32,
    ) {
        self.interaction.shift_active = modifiers.shift;
        self.handle_switching_event(SwitchingEvent::HoldModifiersChanged(
            hold_modifiers_from_state(modifiers),
        ));
    }
}

impl PointerHandler for ProtocolObserver {
    fn pointer_frame(
        &mut self,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
        _pointer: &wl_pointer::WlPointer,
        events: &[PointerEvent],
    ) {
        const PRIMARY_BUTTON: u32 = 0x110;

        for event in events {
            if !self
                .layer
                .as_ref()
                .is_some_and(|layer| layer.wl_surface() == &event.surface)
            {
                continue;
            }
            match event.kind {
                PointerEventKind::Enter { .. } => {
                    let window = self.window_at_pointer(event.position);
                    self.handle_service_event(ServiceEvent::PointerEntered(window));
                }
                PointerEventKind::Motion { .. } => {
                    let window = self.window_at_pointer(event.position);
                    self.handle_service_event(ServiceEvent::PointerMoved(window));
                }
                PointerEventKind::Press { button, .. } if button == PRIMARY_BUTTON => {
                    let window = self.window_at_pointer(event.position);
                    self.handle_service_event(ServiceEvent::PointerPressed(window));
                }
                PointerEventKind::Release { button, .. } if button == PRIMARY_BUTTON => {
                    let window = self.window_at_pointer(event.position);
                    self.handle_service_event(ServiceEvent::PointerReleased(window));
                }
                PointerEventKind::Leave { .. }
                | PointerEventKind::Press { .. }
                | PointerEventKind::Release { .. }
                | PointerEventKind::Axis { .. } => {}
            }
        }
    }
}

impl Dispatch<wp_fractional_scale_v1::WpFractionalScaleV1, ()> for ProtocolObserver {
    fn event(
        state: &mut Self,
        scale: &wp_fractional_scale_v1::WpFractionalScaleV1,
        event: wp_fractional_scale_v1::Event,
        _data: &(),
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
    ) {
        if state.fractional_scale.as_ref() != Some(scale) {
            return;
        }
        let wp_fractional_scale_v1::Event::PreferredScale {
            scale: preferred_scale,
        } = event
        else {
            return;
        };
        let preferred_scale = FractionalScale::from_protocol_units(preferred_scale);
        if state.preferred_scale == Some(preferred_scale) {
            return;
        }
        state.preferred_scale = Some(preferred_scale);
        if state.grid.is_some()
            && let Err(error) = state.render_grid()
        {
            state.fail_overlay("render fractionally scaled Switcher Grid failed", &error);
        }
    }
}

impl Dispatch<wp_viewport::WpViewport, ()> for ProtocolObserver {
    fn event(
        _state: &mut Self,
        _viewport: &wp_viewport::WpViewport,
        _event: wp_viewport::Event,
        _data: &(),
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
    ) {
        unreachable!("wp_viewport has no events");
    }
}

impl ShmHandler for ProtocolObserver {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

impl ShmCaptureHandler for ProtocolObserver {
    fn constraints_ready(
        &mut self,
        _queue_handle: &QueueHandle<Self>,
        session: &CaptureSession,
        constraints: ShmConstraints,
    ) {
        let Some(window) = ShmCaptureState::session_window(session) else {
            return;
        };
        let effects = self.capture_model.initialized(&window, &constraints);
        if let Err(error) = self.apply_capture_effects(effects) {
            let failure_effects = self
                .capture_model
                .failed(&window, CaptureFailure::FrameFailed);
            let _degraded = self.apply_capture_effects(failure_effects);
            eprintln!("initialize Live Thumbnail capture for Window {window} failed: {error:#}");
        }
    }

    fn capture_stopped(&mut self, session: &CaptureSession) {
        let Some(window) = ShmCaptureState::session_window(session) else {
            return;
        };
        let effects = self.capture_model.failed(&window, CaptureFailure::Stopped);
        if let Err(error) = self.apply_capture_effects(effects) {
            eprintln!("degrade stopped Live Thumbnail for Window {window} failed: {error:#}");
        }
    }

    fn frame_ready(&mut self, capture_frame: &CaptureFrame) {
        let completed = match self.capture_backend.complete_frame(capture_frame) {
            Ok(completed) => completed,
            Err(error) => {
                let window = ShmCaptureState::frame_window(capture_frame)
                    .unwrap_or_else(|| WindowId::from("<unknown>"));
                let effects = self
                    .capture_model
                    .failed(&window, CaptureFailure::InvalidDimensions);
                let _degraded = self.apply_capture_effects(effects);
                eprintln!("complete Live Thumbnail for Window {window} failed: {error:#}");
                return;
            }
        };
        let window = completed.window;
        let effects = self.capture_model.frame_ready(
            &window,
            self.capture_clock.elapsed(),
            &completed.damage,
        );
        if !effects
            .iter()
            .any(|effect| matches!(effect, CaptureEffect::PresentThumbnail(_)))
        {
            return;
        }
        let changed = self
            .grid
            .as_mut()
            .is_some_and(|grid| grid.update_thumbnail(&window, completed.thumbnail));
        if changed && let Err(error) = self.render_grid() {
            eprintln!("render Live Thumbnail for Window {window} failed: {error:#}");
        }
    }

    fn frame_failed(&mut self, capture_frame: &CaptureFrame) {
        let Some(window) = ShmCaptureState::frame_window(capture_frame) else {
            self.capture_backend.fail_frame(capture_frame);
            return;
        };
        self.capture_backend.fail_frame(capture_frame);
        let effects = self
            .capture_model
            .failed(&window, CaptureFailure::FrameFailed);
        if let Err(error) = self.apply_capture_effects(effects) {
            eprintln!("degrade failed Live Thumbnail for Window {window} failed: {error:#}");
        }
    }
}

impl ToplevelManagerHandler for ProtocolObserver {
    fn toplevel_manager_state(&mut self) -> &mut ToplevelManagerState {
        &mut self.toplevel_manager
    }

    fn capabilities(
        &mut self,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
        capabilities: Vec<
            WEnum<zcosmic_toplevel_manager_v1::ZcosmicToplelevelManagementCapabilitiesV1>,
        >,
    ) {
        self.management_can_activate = capabilities.iter().any(|capability| {
            *capability
                == WEnum::Value(
                    zcosmic_toplevel_manager_v1::ZcosmicToplelevelManagementCapabilitiesV1::Activate,
                )
        });
    }
}

impl WorkspaceHandler for ProtocolObserver {
    fn workspace_state(&mut self) -> &mut WorkspaceState {
        &mut self.workspace_state
    }

    fn done(&mut self) {
        self.workspace_snapshot_received = true;
        self.update_workspace_eligibility_state();
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Observation {
    Discovered(ObservationKey),
    Identified {
        key: ObservationKey,
        id: WindowId,
    },
    ActivationChanged {
        key: ObservationKey,
        activated: bool,
    },
    Closed(ObservationKey),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ObservationKey(u64);

#[derive(Clone, Debug)]
struct ObservationWindow {
    key: ObservationKey,
    id: Option<WindowId>,
    activated: bool,
    registered: bool,
}

#[derive(Clone, Debug, Default)]
struct ObservationLedger {
    windows: Vec<ObservationWindow>,
    pending_activations: Vec<ObservationKey>,
}

impl ObservationLedger {
    fn window_id(&self, key: ObservationKey) -> Option<&WindowId> {
        self.windows
            .iter()
            .find(|window| window.key == key)
            .and_then(|window| window.id.as_ref())
    }

    fn apply(&mut self, observation: Observation) -> Vec<WindowEvent> {
        let mut events = Vec::new();
        match observation {
            Observation::Discovered(key) => self.windows.push(ObservationWindow {
                key,
                id: None,
                activated: false,
                registered: false,
            }),
            Observation::Identified { key, id } => {
                if let Some(window) = self.windows.iter_mut().find(|window| window.key == key) {
                    window.id = Some(id);
                }
            }
            Observation::ActivationChanged { key, activated } => {
                if let Some(window) = self.windows.iter_mut().find(|window| window.key == key) {
                    if activated && !window.activated {
                        self.pending_activations.push(key);
                    }
                    window.activated = activated;
                }
            }
            Observation::Closed(key) => {
                if let Some(position) = self.windows.iter().position(|window| window.key == key) {
                    let window = self.windows.remove(position);
                    if window.registered
                        && let Some(id) = window.id
                    {
                        events.push(WindowEvent::Closed(id));
                    }
                }
                self.pending_activations
                    .retain(|pending_key| *pending_key != key);
            }
        }

        self.register_ready_windows(&mut events);
        self.replay_ready_activations(&mut events);
        events
    }

    fn register_ready_windows(&mut self, events: &mut Vec<WindowEvent>) {
        for window in &mut self.windows {
            if window.registered {
                continue;
            }
            let Some(id) = window.id.clone() else {
                break;
            };
            window.registered = true;
            events.push(WindowEvent::Discovered(id));
        }
    }

    fn replay_ready_activations(&mut self, events: &mut Vec<WindowEvent>) {
        while let Some(key) = self.pending_activations.first().copied() {
            let Some(window) = self.windows.iter().find(|window| window.key == key) else {
                self.pending_activations.remove(0);
                continue;
            };
            if !window.registered {
                break;
            }
            let id = window
                .id
                .clone()
                .expect("a registered Window has an opaque identity");
            self.pending_activations.remove(0);
            events.push(WindowEvent::Activated(id));
        }
    }
}

impl ProvidesRegistryState for ProtocolObserver {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }

    registry_handlers!(OutputState, SeatState);
}

impl Dispatch<ext_foreign_toplevel_list_v1::ExtForeignToplevelListV1, GlobalData>
    for ProtocolObserver
{
    fn event(
        state: &mut Self,
        list: &ext_foreign_toplevel_list_v1::ExtForeignToplevelListV1,
        event: ext_foreign_toplevel_list_v1::Event,
        _data: &GlobalData,
        _connection: &Connection,
        queue_handle: &QueueHandle<Self>,
    ) {
        match event {
            ext_foreign_toplevel_list_v1::Event::Toplevel { toplevel } => {
                state.await_toplevel_snapshot();
                let cosmic_toplevel = state.cosmic_toplevel_info.as_ref().map(|toplevel_info| {
                    toplevel_info.get_cosmic_toplevel(&toplevel, queue_handle, GlobalData)
                });
                let key = ObservationKey(state.next_observation_key);
                state.next_observation_key += 1;
                state.windows.push(ObservedWindow {
                    key,
                    foreign_toplevel: toplevel,
                    cosmic_toplevel,
                    title: String::new(),
                    application_id: String::new(),
                    outputs: Vec::new(),
                    geometries: std::collections::HashMap::new(),
                    workspaces: std::collections::HashSet::new(),
                    committed_workspaces: std::collections::HashSet::new(),
                    minimized: false,
                    fullscreen: false,
                    sticky: false,
                });
                state.apply(Observation::Discovered(key));
            }
            ext_foreign_toplevel_list_v1::Event::Finished => list.destroy(),
            _ => {}
        }
    }

    wayland_client::event_created_child!(ProtocolObserver, ext_foreign_toplevel_list_v1::ExtForeignToplevelListV1, [
        ext_foreign_toplevel_list_v1::EVT_TOPLEVEL_OPCODE => (ext_foreign_toplevel_handle_v1::ExtForeignToplevelHandleV1, ())
    ]);
}

impl Dispatch<ext_foreign_toplevel_handle_v1::ExtForeignToplevelHandleV1, ()> for ProtocolObserver {
    fn event(
        state: &mut Self,
        toplevel: &ext_foreign_toplevel_handle_v1::ExtForeignToplevelHandleV1,
        event: ext_foreign_toplevel_handle_v1::Event,
        _data: &(),
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
    ) {
        let Some(window) = state
            .windows
            .iter()
            .find(|window| window.foreign_toplevel == *toplevel)
            .cloned()
        else {
            return;
        };
        match event {
            ext_foreign_toplevel_handle_v1::Event::Title { title } => {
                if let Some(window) = state
                    .windows
                    .iter_mut()
                    .find(|candidate| candidate.key == window.key)
                {
                    window.title = title;
                }
            }
            ext_foreign_toplevel_handle_v1::Event::AppId { app_id } => {
                if let Some(window) = state
                    .windows
                    .iter_mut()
                    .find(|candidate| candidate.key == window.key)
                {
                    window.application_id = app_id;
                }
            }
            ext_foreign_toplevel_handle_v1::Event::Identifier { identifier } => {
                state.apply(Observation::Identified {
                    key: window.key,
                    id: WindowId::from(identifier),
                });
            }
            ext_foreign_toplevel_handle_v1::Event::Closed => {
                state.apply(Observation::Closed(window.key));
                state
                    .windows
                    .retain(|candidate| candidate.key != window.key);
            }
            _ => {}
        }
    }
}

impl Dispatch<zcosmic_toplevel_info_v1::ZcosmicToplevelInfoV1, GlobalData> for ProtocolObserver {
    fn event(
        state: &mut Self,
        _info: &zcosmic_toplevel_info_v1::ZcosmicToplevelInfoV1,
        event: zcosmic_toplevel_info_v1::Event,
        _data: &GlobalData,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
    ) {
        if let zcosmic_toplevel_info_v1::Event::Done = event {
            state.commit_toplevel_snapshot();
        }
    }

    wayland_client::event_created_child!(ProtocolObserver, zcosmic_toplevel_info_v1::ZcosmicToplevelInfoV1, [
        zcosmic_toplevel_info_v1::EVT_TOPLEVEL_OPCODE => (zcosmic_toplevel_handle_v1::ZcosmicToplevelHandleV1, GlobalData)
    ]);
}

impl Dispatch<zcosmic_toplevel_handle_v1::ZcosmicToplevelHandleV1, GlobalData>
    for ProtocolObserver
{
    fn event(
        state: &mut Self,
        toplevel: &zcosmic_toplevel_handle_v1::ZcosmicToplevelHandleV1,
        event: zcosmic_toplevel_handle_v1::Event,
        _data: &GlobalData,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
    ) {
        let Some(key) = state
            .windows
            .iter()
            .find(|window| window.cosmic_toplevel.as_ref() == Some(toplevel))
            .map(|window| window.key)
        else {
            return;
        };
        state.await_toplevel_snapshot();
        match event {
            zcosmic_toplevel_handle_v1::Event::OutputEnter { output } => {
                if let Some(window) = state.windows.iter_mut().find(|window| window.key == key)
                    && !window.outputs.contains(&output)
                {
                    window.outputs.push(output);
                }
            }
            zcosmic_toplevel_handle_v1::Event::OutputLeave { output } => {
                if let Some(window) = state.windows.iter_mut().find(|window| window.key == key) {
                    window.outputs.retain(|candidate| *candidate != output);
                    window.geometries.remove(&output);
                }
            }
            zcosmic_toplevel_handle_v1::Event::Geometry {
                output,
                x,
                y,
                width,
                height,
            } => {
                if let Some(window) = state.windows.iter_mut().find(|window| window.key == key) {
                    window.geometries.insert(
                        output,
                        WindowGeometry {
                            x,
                            y,
                            width,
                            height,
                        },
                    );
                }
            }
            zcosmic_toplevel_handle_v1::Event::State { state: states } => {
                if let Some(window) = state.windows.iter_mut().find(|window| window.key == key) {
                    window.minimized =
                        toplevel_has_state(&states, zcosmic_toplevel_handle_v1::State::Minimized);
                    window.fullscreen =
                        toplevel_has_state(&states, zcosmic_toplevel_handle_v1::State::Fullscreen);
                    window.sticky =
                        toplevel_has_state(&states, zcosmic_toplevel_handle_v1::State::Sticky);
                }
                state.apply(Observation::ActivationChanged {
                    key,
                    activated: toplevel_has_state(
                        &states,
                        zcosmic_toplevel_handle_v1::State::Activated,
                    ),
                });
            }
            zcosmic_toplevel_handle_v1::Event::ExtWorkspaceEnter { workspace } => {
                if let Some(window) = state.windows.iter_mut().find(|window| window.key == key) {
                    window.workspaces.insert(workspace);
                }
            }
            zcosmic_toplevel_handle_v1::Event::ExtWorkspaceLeave { workspace } => {
                if let Some(window) = state.windows.iter_mut().find(|window| window.key == key) {
                    window.workspaces.remove(&workspace);
                }
            }
            _ => {}
        }
    }
}

fn toplevel_has_state(bytes: &[u8], expected: zcosmic_toplevel_handle_v1::State) -> bool {
    bytes.chunks_exact(4).any(|bytes| {
        zcosmic_toplevel_handle_v1::State::try_from(u32::from_ne_bytes(
            bytes.try_into().expect("state chunks contain four bytes"),
        ))
        .is_ok_and(|state| state == expected)
    })
}

fn hold_modifiers_from_keysyms(keysyms: &[Keysym]) -> HoldModifiers {
    keysyms
        .iter()
        .fold(HoldModifiers::empty(), |modifiers, keysym| {
            modifiers | hold_modifier_from_keysym(*keysym)
        })
}

fn hold_modifier_from_keysym(keysym: Keysym) -> HoldModifiers {
    match keysym {
        Keysym::Alt_L | Keysym::Alt_R | Keysym::Meta_L | Keysym::Meta_R => HoldModifiers::ALT,
        Keysym::Control_L | Keysym::Control_R => HoldModifiers::CONTROL,
        Keysym::Super_L | Keysym::Super_R => HoldModifiers::SUPER,
        _ => HoldModifiers::empty(),
    }
}

fn hold_modifiers_from_state(modifiers: Modifiers) -> HoldModifiers {
    let mut held = HoldModifiers::empty();
    if modifiers.alt {
        held = held | HoldModifiers::ALT;
    }
    if modifiers.ctrl {
        held = held | HoldModifiers::CONTROL;
    }
    if modifiers.logo {
        held = held | HoldModifiers::SUPER;
    }
    held
}

delegate_compositor!(ProtocolObserver);
delegate_keyboard!(ProtocolObserver);
delegate_pointer!(ProtocolObserver);
delegate_layer!(ProtocolObserver);
delegate_output!(ProtocolObserver);
delegate_registry!(ProtocolObserver);
delegate_seat!(ProtocolObserver);
delegate_shm!(ProtocolObserver);
wayland_client::delegate_dispatch!(ProtocolObserver: [
    ext_image_copy_capture_manager_v1::ExtImageCopyCaptureManagerV1: GlobalData
] => ShmCaptureState);
wayland_client::delegate_dispatch!(ProtocolObserver: [
    ext_foreign_toplevel_image_capture_source_manager_v1::ExtForeignToplevelImageCaptureSourceManagerV1: GlobalData
] => ShmCaptureState);
wayland_client::delegate_dispatch!(ProtocolObserver: [
    ext_image_capture_source_v1::ExtImageCaptureSourceV1: GlobalData
] => ShmCaptureState);
wayland_client::delegate_dispatch!(ProtocolObserver: [CaptureSession: CaptureSessionData] => ShmCaptureState);
wayland_client::delegate_dispatch!(ProtocolObserver: [CaptureFrame: CaptureFrameData] => ShmCaptureState);
cosmic_client_toolkit::delegate_toplevel_manager!(ProtocolObserver);
cosmic_client_toolkit::delegate_workspace!(ProtocolObserver);
delegate_noop!(ProtocolObserver: ignore wl_buffer::WlBuffer);
delegate_noop!(ProtocolObserver: ignore wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1);
delegate_noop!(ProtocolObserver: ignore wp_viewporter::WpViewporter);

#[cfg(test)]
mod tests {
    use std::sync::{Arc, RwLock};

    use cosmic_window_switcher::{
        MruHistoryAccuracy, ServiceDiagnostics, SwitcherService, WindowId, WindowScope,
        WorkspaceEligibilityState,
    };

    use super::{Observation, ObservationKey, ObservationLedger};

    #[test]
    fn activation_before_delayed_identity_preserves_observed_recency() {
        let service = Arc::new(RwLock::new(SwitcherService::new()));
        let mut observations = ObservationLedger::default();
        let scenario = [
            Observation::Discovered(ObservationKey(1)),
            Observation::Discovered(ObservationKey(2)),
            Observation::Identified {
                key: ObservationKey(2),
                id: WindowId::from("activated"),
            },
            Observation::ActivationChanged {
                key: ObservationKey(2),
                activated: true,
            },
            Observation::ActivationChanged {
                key: ObservationKey(2),
                activated: false,
            },
            Observation::Identified {
                key: ObservationKey(1),
                id: WindowId::from("delayed"),
            },
        ];

        for observation in scenario {
            let events = observations.apply(observation);
            let mut service = service.write().expect("scenario service lock is available");
            for event in events {
                service.observe(event);
            }
        }

        assert_eq!(
            service
                .read()
                .expect("scenario service lock is available")
                .diagnostics(),
            ServiceDiagnostics {
                mru_history: MruHistoryAccuracy::WarmUp,
                mru_order: vec![WindowId::from("activated"), WindowId::from("delayed")],
                window_scope: WindowScope::AllWorkspaces,
                workspace_eligibility: WorkspaceEligibilityState::AwaitingSnapshot,
            }
        );
    }
}
