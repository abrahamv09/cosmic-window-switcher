// SPDX-License-Identifier: GPL-3.0-only

use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use cosmic_client_toolkit::{
    GlobalData,
    toplevel_management::{ToplevelManagerHandler, ToplevelManagerState},
};
use cosmic_protocols::toplevel_info::v1::client::{
    zcosmic_toplevel_handle_v1, zcosmic_toplevel_info_v1,
};
use cosmic_protocols::toplevel_management::v1::client::zcosmic_toplevel_manager_v1;
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    delegate_compositor, delegate_keyboard, delegate_layer, delegate_output, delegate_registry,
    delegate_seat, delegate_shm,
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{
        Capability, SeatHandler, SeatState,
        keyboard::{KeyEvent, KeyboardHandler, Keysym, Modifiers, RawModifiers},
    },
    shell::{
        WaylandSurface,
        wlr_layer::{
            KeyboardInteractivity, Layer, LayerShell, LayerShellHandler, LayerSurface,
            LayerSurfaceConfigure,
        },
    },
    shm::{Shm, ShmHandler, raw::RawPool},
};
use wayland_client::{
    Connection, Dispatch, EventQueue, QueueHandle, WEnum, delegate_noop,
    globals::registry_queue_init,
    protocol::{wl_buffer, wl_keyboard, wl_output, wl_seat, wl_shm, wl_surface},
};
use wayland_protocols::ext::foreign_toplevel_list::v1::client::{
    ext_foreign_toplevel_handle_v1, ext_foreign_toplevel_list_v1,
};

use cosmic_window_switcher::{
    APPLICATION_ID, HoldModifiers, InvocationDirection, InvocationRequest, ServiceEffect,
    ServiceEvent, SessionDisplay, SwitcherCard, SwitcherGrid, SwitchingEvent, WindowEvent,
    WindowId,
};

use super::{
    PendingInvocations, SharedService, invocation,
    overlay::{OverlayRenderer, RenderedOverlay},
};

const REVEAL_DELAY: Duration = Duration::from_millis(100);
const SESSION_READINESS_TIMEOUT: Duration = Duration::from_millis(500);

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
        let compositor =
            CompositorState::bind(&globals, &queue_handle).context("bind wl_compositor")?;
        let layer_shell =
            LayerShell::bind(&globals, &queue_handle).context("bind wlr layer shell")?;
        let shm = Shm::bind(&globals, &queue_handle).context("bind wl_shm")?;
        let registry_state = RegistryState::new(&globals);
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
            .context("the compositor does not expose COSMIC Window state")?;
        let toplevel_manager = ToplevelManagerState::try_new(&registry_state, &queue_handle)
            .context("the compositor does not expose COSMIC Window management")?;
        let state = ProtocolObserver {
            queue_handle: queue_handle.clone(),
            registry_state,
            compositor,
            layer_shell,
            shm,
            output_state: OutputState::new(&globals, &queue_handle),
            seat_state: SeatState::new(&globals, &queue_handle),
            toplevel_manager,
            _foreign_toplevel_list: foreign_toplevel_list,
            cosmic_toplevel_info,
            windows: Vec::new(),
            observations: ObservationLedger::default(),
            next_observation_key: 0,
            service,
            management_can_activate: false,
            overlay_renderer: OverlayRenderer::new(),
            layer: None,
            overlay_pool: None,
            overlay_buffer: None,
            visible_pool: None,
            visible_buffer: None,
            visible_dimensions: None,
            grid: None,
            session_window_order: Vec::new(),
            session_output: None,
            interaction: InteractionState::default(),
            keyboard: None,
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
        self.event_queue
            .roundtrip(&mut self.state)
            .context("receive initial COSMIC Window state")?;
        self.event_queue
            .roundtrip(&mut self.state)
            .context("finish initial COSMIC Window state")?;
        self.event_queue
            .roundtrip(&mut self.state)
            .context("synchronize initial COSMIC Window state")?;
        self.state
            .service
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .complete_initial_discovery();
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
        Ok(())
    }
}

#[derive(Clone)]
struct ObservedWindow {
    key: ObservationKey,
    foreign_toplevel: ext_foreign_toplevel_handle_v1::ExtForeignToplevelHandleV1,
    cosmic_toplevel: zcosmic_toplevel_handle_v1::ZcosmicToplevelHandleV1,
    title: String,
    application_id: String,
    outputs: Vec<wl_output::WlOutput>,
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
    seat_state: SeatState,
    toplevel_manager: ToplevelManagerState,
    _foreign_toplevel_list: ext_foreign_toplevel_list_v1::ExtForeignToplevelListV1,
    cosmic_toplevel_info: zcosmic_toplevel_info_v1::ZcosmicToplevelInfoV1,
    windows: Vec<ObservedWindow>,
    observations: ObservationLedger,
    next_observation_key: u64,
    service: SharedService,
    management_can_activate: bool,
    overlay_renderer: OverlayRenderer,
    layer: Option<LayerSurface>,
    overlay_pool: Option<RawPool>,
    overlay_buffer: Option<wl_buffer::WlBuffer>,
    visible_pool: Option<RawPool>,
    visible_buffer: Option<wl_buffer::WlBuffer>,
    visible_dimensions: Option<(u32, u32, i32)>,
    grid: Option<SwitcherGrid>,
    session_window_order: Vec<WindowId>,
    session_output: Option<wl_output::WlOutput>,
    interaction: InteractionState,
    keyboard: Option<wl_keyboard::WlKeyboard>,
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
        if grid_changed
            && self.grid.is_some()
            && let Err(error) = self.render_grid()
        {
            eprintln!("render Switcher Grid after Window closure failed: {error:#}");
            if let Some(direction) = self.pending_direction {
                self.fallback(direction);
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

        let mru_order = self
            .service
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .diagnostics()
            .mru_order;
        if mru_order.len() < 2 {
            return;
        }
        if !self.management_can_activate || self.seat.is_none() {
            self.fallback(direction);
            return;
        }
        let Some(session_output) = mru_order
            .first()
            .and_then(|focused| self.observed_window(focused))
            .and_then(|window| window.outputs.first())
            .cloned()
        else {
            self.fallback(direction);
            return;
        };

        let surface = self.compositor.create_surface(queue_handle);
        let layer = self.layer_shell.create_layer_surface(
            queue_handle,
            surface,
            Layer::Overlay,
            Some(APPLICATION_ID),
            Some(&session_output),
        );
        layer.set_keyboard_interactivity(KeyboardInteractivity::Exclusive);
        layer.set_size(1, 1);
        layer.commit();
        self.layer = Some(layer);
        self.session_window_order = mru_order;
        self.session_output = Some(session_output);
        self.grid = None;
        self.interaction = InteractionState::default();
        self.pending_direction = Some(direction);
        self.initial_hold_modifiers = None;
        let now = Instant::now();
        self.reveal_at = Some(now + REVEAL_DELAY);
        self.readiness_deadline = Some(now + SESSION_READINESS_TIMEOUT);
    }

    fn poll_timeout(&self) -> Option<Duration> {
        [self.reveal_at, self.readiness_deadline]
            .into_iter()
            .flatten()
            .min()
            .map(|deadline| deadline.saturating_duration_since(Instant::now()))
    }

    fn try_mark_ready(&mut self) {
        if !self.interaction.keyboard_focused
            || self.visible_pool.is_none()
            || self.visible_buffer.is_none()
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
        let effects = self
            .service
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .handle(service_event);
        self.apply_effects(effects);
    }

    fn apply_effects(&mut self, effects: Vec<ServiceEffect>) {
        for effect in effects {
            match effect {
                ServiceEffect::PrepareInvisibleOverlay { selected } => {
                    if let Err(error) = self.prepare_grid(&selected) {
                        eprintln!("prepare Switcher Grid failed: {error:#}");
                        if let Some(direction) = self.pending_direction {
                            self.fallback(direction);
                        }
                    }
                }
                ServiceEffect::SelectionChanged(selected) => {
                    if let Err(error) = self.select_grid(&selected) {
                        eprintln!("render Switcher Grid selection failed: {error:#}");
                        if let Some(direction) = self.pending_direction {
                            self.fallback(direction);
                        }
                    }
                }
                ServiceEffect::RevealOverlay { selected } => {
                    if let Some(grid) = self.grid.as_mut() {
                        let _selection = grid.select(&selected);
                    }
                    if let Err(error) = self.show_grid() {
                        eprintln!("reveal Switcher Grid failed: {error:#}");
                        if let Some(direction) = self.pending_direction {
                            self.fallback(direction);
                        }
                    }
                }
                ServiceEffect::Activate(window) => {
                    let target = self
                        .windows
                        .iter()
                        .find(|candidate| {
                            self.observations.window_id(candidate.key) == Some(&window)
                        })
                        .map(|candidate| candidate.cosmic_toplevel.clone());
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

    fn destroy_overlay(&mut self) {
        if let Some(layer) = self.layer.take() {
            layer.set_keyboard_interactivity(KeyboardInteractivity::None);
            layer.commit();
        }
        if let Some(buffer) = self.visible_buffer.take() {
            buffer.destroy();
        }
        if let Some(buffer) = self.overlay_buffer.take() {
            buffer.destroy();
        }
        self.overlay_pool = None;
        self.visible_pool = None;
        self.visible_dimensions = None;
        self.grid = None;
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
        let cards = self
            .session_window_order
            .iter()
            .map(|id| {
                self.observed_window(id).map(|window| {
                    SwitcherCard::new(
                        id.clone(),
                        window.application_id.clone(),
                        window.title.clone(),
                    )
                })
            })
            .collect::<Option<Vec<_>>>()
            .context("a Session Window is missing icon-and-title metadata")?;
        self.grid = Some(
            SwitcherGrid::new(session_display, cards, selected)
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
        self.render_grid()
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
        let maximum_width = output_info
            .logical_size
            .and_then(|(width, _)| u32::try_from(width).ok())
            .map_or(1_200, |width| width.saturating_mul(4) / 5);
        let rendered = self.overlay_renderer.render(
            self.grid
                .as_ref()
                .context("the Switching Session has no Switcher Grid")?,
            maximum_width,
            output_info.scale_factor,
        )?;
        self.install_rendered_grid(rendered)
    }

    fn install_rendered_grid(&mut self, rendered: RenderedOverlay) -> Result<()> {
        let RenderedOverlay {
            logical_width,
            logical_height,
            scale,
            pixels,
        } = rendered;
        let mut pool = RawPool::new(pixels.len(), &self.shm)
            .context("allocate shared memory for the Switcher Grid")?;
        pool.mmap().copy_from_slice(&pixels);
        let scale_u32 = u32::try_from(scale).context("invalid output scale")?;
        let physical_width = logical_width
            .checked_mul(scale_u32)
            .context("Switcher Grid width overflow")?;
        let physical_height = logical_height
            .checked_mul(scale_u32)
            .context("Switcher Grid height overflow")?;
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
        if let Some(previous) = self.visible_buffer.replace(buffer) {
            previous.destroy();
        }
        self.visible_pool = Some(pool);
        self.visible_dimensions = Some((logical_width, logical_height, scale));
        if self.interaction.visible {
            self.attach_visible_buffer()?;
        }
        self.try_mark_ready();
        Ok(())
    }

    fn show_grid(&mut self) -> Result<()> {
        self.interaction.visible = true;
        self.attach_visible_buffer()
    }

    fn attach_visible_buffer(&self) -> Result<()> {
        let layer = self
            .layer
            .as_ref()
            .context("the Switching Session has no layer surface")?;
        let buffer = self
            .visible_buffer
            .as_ref()
            .context("the Switcher Grid has no rendered buffer")?;
        let (logical_width, logical_height, scale) = self
            .visible_dimensions
            .context("the Switcher Grid has no rendered dimensions")?;
        let scale_u32 = u32::try_from(scale).context("invalid output scale")?;
        layer.set_size(logical_width, logical_height);
        layer.wl_surface().set_buffer_scale(scale);
        layer.wl_surface().attach(Some(buffer), 0, 0);
        layer.wl_surface().damage_buffer(
            0,
            0,
            i32::try_from(
                logical_width
                    .checked_mul(scale_u32)
                    .context("Switcher Grid width overflow")?,
            )
            .context("Switcher Grid is too wide")?,
            i32::try_from(
                logical_height
                    .checked_mul(scale_u32)
                    .context("Switcher Grid height overflow")?,
            )
            .context("Switcher Grid is too tall")?,
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
        _surface: &wl_surface::WlSurface,
        _new_factor: i32,
    ) {
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
        if self.overlay_pool.is_some() || self.layer.as_ref() != Some(layer) {
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
                self.overlay_buffer = Some(buffer);
                self.overlay_pool = Some(pool);
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
            .invoke(InvocationRequest {
                direction,
                initial_hold_modifiers: modifiers,
            });
        self.apply_effects(effects);
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

impl ShmHandler for ProtocolObserver {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
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
                let cosmic_toplevel = state.cosmic_toplevel_info.get_cosmic_toplevel(
                    &toplevel,
                    queue_handle,
                    GlobalData,
                );
                let key = ObservationKey(state.next_observation_key);
                state.next_observation_key += 1;
                state.windows.push(ObservedWindow {
                    key,
                    foreign_toplevel: toplevel,
                    cosmic_toplevel,
                    title: String::new(),
                    application_id: String::new(),
                    outputs: Vec::new(),
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
        _state: &mut Self,
        _info: &zcosmic_toplevel_info_v1::ZcosmicToplevelInfoV1,
        _event: zcosmic_toplevel_info_v1::Event,
        _data: &GlobalData,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
    ) {
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
            .find(|window| window.cosmic_toplevel == *toplevel)
            .map(|window| window.key)
        else {
            return;
        };
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
                }
            }
            zcosmic_toplevel_handle_v1::Event::State { state: states } => {
                state.apply(Observation::ActivationChanged {
                    key,
                    activated: toplevel_is_activated(&states),
                });
            }
            _ => {}
        }
    }
}

fn toplevel_is_activated(bytes: &[u8]) -> bool {
    bytes.chunks_exact(4).any(|bytes| {
        zcosmic_toplevel_handle_v1::State::try_from(u32::from_ne_bytes(
            bytes.try_into().expect("state chunks contain four bytes"),
        ))
        .is_ok_and(|state| state == zcosmic_toplevel_handle_v1::State::Activated)
    })
}

fn duration_to_timespec(duration: Duration) -> rustix::event::Timespec {
    rustix::event::Timespec {
        tv_sec: duration
            .as_secs()
            .try_into()
            .expect("a Switching Session deadline fits in seconds"),
        tv_nsec: duration.subsec_nanos().into(),
    }
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
delegate_layer!(ProtocolObserver);
delegate_output!(ProtocolObserver);
delegate_registry!(ProtocolObserver);
delegate_seat!(ProtocolObserver);
delegate_shm!(ProtocolObserver);
cosmic_client_toolkit::delegate_toplevel_manager!(ProtocolObserver);
delegate_noop!(ProtocolObserver: ignore wl_buffer::WlBuffer);

#[cfg(test)]
mod tests {
    use std::sync::{Arc, RwLock};

    use cosmic_window_switcher::{
        MruHistoryAccuracy, ServiceDiagnostics, SwitcherService, WindowId,
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
            }
        );
    }
}
