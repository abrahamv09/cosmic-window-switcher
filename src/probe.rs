// SPDX-License-Identifier: GPL-3.0-only

use std::{
    collections::{HashMap, HashSet},
    time::{Duration, Instant},
};

use anyhow::{Context, Result, bail};
use cosmic_client_toolkit::{
    GlobalData,
    toplevel_management::{ToplevelManagerHandler, ToplevelManagerState},
};
use cosmic_protocols::{
    toplevel_info::v1::client::{zcosmic_toplevel_handle_v1, zcosmic_toplevel_info_v1},
    toplevel_management::v1::client::zcosmic_toplevel_manager_v1,
};
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
    Connection, Dispatch, QueueHandle, WEnum,
    backend::WaylandError,
    delegate_noop,
    globals::registry_queue_init,
    protocol::{wl_buffer, wl_keyboard, wl_output, wl_seat, wl_shm, wl_surface},
};
use wayland_protocols::ext::foreign_toplevel_list::v1::client::{
    ext_foreign_toplevel_handle_v1, ext_foreign_toplevel_list_v1,
};
use wayland_protocols::ext::{
    image_capture_source::v1::client::{
        ext_foreign_toplevel_image_capture_source_manager_v1, ext_image_capture_source_v1,
    },
    image_copy_capture::v1::client::ext_image_copy_capture_manager_v1,
};

use crate::shm_capture::{
    CaptureFrame, CaptureFrameData, CaptureSession, CaptureSessionData, ShmCaptureHandler,
    ShmCaptureState, duration_to_timespec,
};
use cosmic_window_switcher::{
    APPLICATION_ID, HoldModifiers, InvocationDirection, SessionEffect, ShmConstraints,
    ShmFrameLayout, SwitchingEvent, SwitchingSession, WindowId,
};

const LIVE_CONTRACT_FRAME_LIMIT: usize = 3;
const LIVE_CONTRACT_DURATION: Duration = Duration::from_secs(10);

pub fn run(include_titles: bool, live_thumbnails: bool) -> Result<()> {
    crate::cosmic_session::verify("probe")?;

    let connection = Connection::connect_to_env().context("connect to the Wayland compositor")?;
    let (globals, mut event_queue) =
        registry_queue_init(&connection).context("read Wayland globals")?;
    let queue_handle = event_queue.handle();

    let compositor =
        CompositorState::bind(&globals, &queue_handle).context("bind wl_compositor")?;
    let layer_shell = LayerShell::bind(&globals, &queue_handle).context("bind wlr layer shell")?;
    let shm = Shm::bind(&globals, &queue_handle).context("bind wl_shm")?;
    let registry_state = RegistryState::new(&globals);
    let _foreign_toplevel_list = registry_state
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

    let mut probe = Probe {
        registry_state,
        output_state: OutputState::new(&globals, &queue_handle),
        seat_state: SeatState::new(&globals, &queue_handle),
        shm,
        cosmic_toplevel_info,
        windows: Vec::new(),
        capture_backend: ShmCaptureState::new(&globals, &queue_handle),
        toplevel_manager,
        layer: None,
        overlay_pool: None,
        keyboard: None,
        seat: None,
        session: None,
        initial_hold_modifiers: None,
        pending_switching_events: Vec::new(),
        capture_windows: Vec::new(),
        pending_recaptures: Vec::new(),
        captured_frames: HashMap::new(),
        unchanged_windows: HashSet::new(),
        capture_attempts: 0,
        capture_failures: 0,
        capture_succeeded: false,
        capture_mode: CaptureProbeMode::from(live_thumbnails),
        terminal_input: false,
        management_can_activate: false,
        title_visibility: TitleVisibility::from(include_titles),
        activation_target: None,
        activation_requests: 0,
        fatal_error: None,
    };

    event_queue
        .roundtrip(&mut probe)
        .context("receive initial COSMIC Window state")?;
    event_queue
        .roundtrip(&mut probe)
        .context("finish initial COSMIC Window state")?;
    event_queue
        .roundtrip(&mut probe)
        .context("synchronize initial COSMIC Window state")?;

    probe.layer = Some(create_keyboard_overlay(
        &compositor,
        &layer_shell,
        &queue_handle,
    ));

    probe.start_capture(&queue_handle)?;
    probe.try_start_switching_session()?;
    println!(
        "Transparent keyboard overlay ready. Press Tab to cycle, Escape to cancel, or release the initial Alt/Ctrl/Super modifier to activate."
    );

    let live_contract_deadline = live_thumbnails.then(|| Instant::now() + LIVE_CONTRACT_DURATION);
    while !probe.is_finished() {
        if live_contract_deadline.is_some_and(|deadline| Instant::now() >= deadline) {
            probe.handle_switching_event(SwitchingEvent::Escape);
            break;
        }
        if let Some(deadline) = live_contract_deadline {
            dispatch_until(&mut event_queue, &mut probe, deadline)?;
        } else {
            event_queue
                .blocking_dispatch(&mut probe)
                .context("dispatch COSMIC probe event")?;
        }
        probe.start_pending_recaptures(&queue_handle);
        connection
            .flush()
            .context("flush requests to the COSMIC compositor")?;
    }

    probe.into_result()
}

fn dispatch_until(
    event_queue: &mut wayland_client::EventQueue<Probe>,
    probe: &mut Probe,
    deadline: Instant,
) -> Result<()> {
    event_queue
        .dispatch_pending(probe)
        .context("dispatch pending COSMIC probe events")?;
    let Some(read_guard) = event_queue.prepare_read() else {
        return Ok(());
    };
    let fd = read_guard.connection_fd();
    let mut poll_fd = rustix::event::PollFd::new(
        &fd,
        rustix::event::PollFlags::IN | rustix::event::PollFlags::ERR,
    );
    let timeout = duration_to_timespec(deadline.saturating_duration_since(Instant::now()));
    rustix::event::poll(std::slice::from_mut(&mut poll_fd), Some(&timeout))
        .context("poll live capture contract events")?;
    if poll_fd
        .revents()
        .intersects(rustix::event::PollFlags::IN | rustix::event::PollFlags::ERR)
    {
        match read_guard.read() {
            Ok(_) => {}
            Err(WaylandError::Io(error)) if error.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(error) => return Err(error).context("read live capture contract events"),
        }
    } else {
        drop(read_guard);
    }
    event_queue
        .dispatch_pending(probe)
        .context("dispatch live capture contract events")?;
    Ok(())
}

fn create_keyboard_overlay(
    compositor: &CompositorState,
    layer_shell: &LayerShell,
    queue_handle: &QueueHandle<Probe>,
) -> LayerSurface {
    let surface = compositor.create_surface(queue_handle);
    let layer = layer_shell.create_layer_surface(
        queue_handle,
        surface,
        Layer::Overlay,
        Some(APPLICATION_ID),
        None,
    );
    layer.set_keyboard_interactivity(KeyboardInteractivity::Exclusive);
    layer.set_size(1, 1);
    layer.commit();
    layer
}

struct Probe {
    registry_state: RegistryState,
    output_state: OutputState,
    seat_state: SeatState,
    shm: Shm,
    cosmic_toplevel_info: zcosmic_toplevel_info_v1::ZcosmicToplevelInfoV1,
    windows: Vec<WindowInfo>,
    capture_backend: ShmCaptureState,
    toplevel_manager: ToplevelManagerState,
    layer: Option<LayerSurface>,
    overlay_pool: Option<RawPool>,
    keyboard: Option<wl_keyboard::WlKeyboard>,
    seat: Option<wl_seat::WlSeat>,
    session: Option<SwitchingSession>,
    initial_hold_modifiers: Option<HoldModifiers>,
    pending_switching_events: Vec<SwitchingEvent>,
    capture_windows: Vec<WindowId>,
    pending_recaptures: Vec<PendingCaptureRequest>,
    captured_frames: HashMap<WindowId, usize>,
    unchanged_windows: HashSet<WindowId>,
    capture_attempts: usize,
    capture_failures: usize,
    capture_succeeded: bool,
    capture_mode: CaptureProbeMode,
    terminal_input: bool,
    management_can_activate: bool,
    title_visibility: TitleVisibility,
    activation_target: Option<WindowId>,
    activation_requests: usize,
    fatal_error: Option<String>,
}

impl Probe {
    fn into_result(mut self) -> Result<()> {
        let result = if let Some(message) = self.fatal_error.take() {
            Err(anyhow::anyhow!(message))
        } else if !self.capture_succeeded {
            Err(anyhow::anyhow!(
                "no SHM frame was captured ({} attempt(s), {} failure(s)); all Windows remained discoverable",
                self.capture_attempts,
                self.capture_failures
            ))
        } else {
            Ok(())
        };
        if self.capture_mode == CaptureProbeMode::LiveContract {
            for window in &self.capture_windows {
                let frames = self.captured_frames.get(window).copied().unwrap_or(0);
                if frames > 1 {
                    println!(
                        "Damage contract Window {window}: {frames} frame(s); changed content produced a new frame."
                    );
                } else if self.unchanged_windows.contains(window) {
                    println!(
                        "Damage contract Window {window}: unchanged compositor response was suppressed."
                    );
                } else {
                    println!(
                        "Damage contract Window {window}: {frames} frame(s); no duplicate frame arrived without compositor damage."
                    );
                }
            }
        }
        self.pending_recaptures.clear();
        let (released_sessions, released_frames, released_allocations) =
            self.capture_backend.stop_all();
        if self.capture_mode == CaptureProbeMode::LiveContract {
            println!(
                "Session-stop contract: released {released_sessions} capture session(s), \
                 {released_frames} outstanding capture frame(s), and \
                 {released_allocations} SHM allocation(s)."
            );
        }
        result
    }

    fn start_pending_recaptures(&mut self, queue_handle: &QueueHandle<Self>) {
        for pending in std::mem::take(&mut self.pending_recaptures) {
            if let Err(error) = self.capture_backend.request_frame(
                &pending.window,
                pending.layout,
                &self.shm,
                queue_handle,
            ) {
                self.note_capture_failure(&format!(
                    "request another SHM frame for Window {} failed: {error:#}",
                    pending.window
                ));
            }
        }
    }

    fn start_capture(&mut self, queue_handle: &QueueHandle<Self>) -> Result<()> {
        let windows = self
            .windows
            .iter()
            .filter(|window| window.metadata_complete)
            .cloned()
            .collect::<Vec<_>>();
        if windows.len() < 2 {
            bail!(
                "the probe requires at least two compositor-managed Windows; discovered {}",
                windows.len()
            );
        }

        let mut identifiers = std::collections::HashSet::new();
        for window in &windows {
            if window.identifier.is_empty() || !identifiers.insert(window.identifier.clone()) {
                bail!("the compositor did not provide distinct opaque Window identities");
            }
            if self.title_visibility == TitleVisibility::Included {
                println!(
                    "Window id={} app_id={:?} title={:?}",
                    window.identifier, window.app_id, window.title
                );
            } else {
                println!("Window id={} app_id={:?}", window.identifier, window.app_id);
            }
        }

        if !self.management_can_activate {
            bail!("the compositor does not advertise Window activation");
        }

        for window in windows {
            let window_id = WindowId::from(window.identifier.clone());
            self.capture_backend
                .create_stream(window_id.clone(), &window.foreign_toplevel, queue_handle)
                .with_context(|| {
                    format!("create capture session for Window {}", window.identifier)
                })?;
            self.capture_windows.push(window_id);
            self.capture_attempts += 1;
        }

        Ok(())
    }

    fn try_start_switching_session(&mut self) -> Result<()> {
        if self.session.is_some() {
            return Ok(());
        }
        let Some(initial_hold_modifiers) = self.initial_hold_modifiers else {
            return Ok(());
        };

        let windows = self
            .windows
            .iter()
            .filter(|window| window.metadata_complete)
            .map(|window| WindowId::from(window.identifier.clone()))
            .collect::<Vec<_>>();
        if windows.len() < 2 {
            return Ok(());
        }

        self.session = Some(
            SwitchingSession::new(windows, InvocationDirection::Next, initial_hold_modifiers)
                .context("start two-Window Switching Session")?,
        );

        let selected = self
            .session
            .as_ref()
            .expect("session just created")
            .selected();
        println!("Selected Window id={selected}");
        if initial_hold_modifiers.is_empty() {
            println!(
                "No Alt, Ctrl, or Super key was held on entry; the probe remains open until Escape."
            );
        }
        for event in std::mem::take(&mut self.pending_switching_events) {
            self.handle_switching_event(event);
        }
        Ok(())
    }

    fn handle_switching_event(&mut self, event: SwitchingEvent) {
        let Some(session) = self.session.as_mut() else {
            self.pending_switching_events.push(event);
            return;
        };
        let effect = session.handle(event);
        self.apply_effect(effect);
    }

    fn apply_effect(&mut self, effect: SessionEffect) {
        match effect {
            SessionEffect::None => {}
            SessionEffect::SelectionChanged(identifier) => {
                println!("Selected Window id={identifier}");
            }
            SessionEffect::Cancelled => {
                println!("Cancelled; the originally focused Window was not changed.");
                self.terminal_input = true;
            }
            SessionEffect::Activate(identifier) => {
                let target = self
                    .windows
                    .iter()
                    .find(|window| window.identifier == identifier.as_str())
                    .and_then(|window| window.cosmic_toplevel.clone());
                match (target, self.seat.as_ref()) {
                    (Some(target), Some(seat)) => {
                        self.activation_requests += 1;
                        if self.activation_requests != 1 {
                            self.fatal_error =
                                Some("more than one Window activation was requested".to_owned());
                            return;
                        }
                        if let Some(layer) = self.layer.as_ref() {
                            layer.set_keyboard_interactivity(KeyboardInteractivity::None);
                            layer.commit();
                        }
                        self.toplevel_manager.manager.activate(&target, seat);
                        self.activation_target = Some(identifier.clone());
                        println!("Requested activation of Window id={identifier} exactly once.");
                    }
                    _ => {
                        self.fatal_error =
                            Some(format!("selected Window {identifier} cannot be activated"));
                    }
                }
            }
        }
    }

    fn is_finished(&self) -> bool {
        self.fatal_error.is_some()
            || (self.terminal_input
                && (self.capture_succeeded || self.capture_failures >= self.capture_attempts))
    }

    fn note_capture_failure(&mut self, message: &str) {
        self.capture_failures += 1;
        eprintln!("{message}; the Window remains in discovery results.");
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TitleVisibility {
    Redacted,
    Included,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CaptureProbeMode {
    SingleFrame,
    LiveContract,
}

impl From<bool> for CaptureProbeMode {
    fn from(live_thumbnails: bool) -> Self {
        if live_thumbnails {
            Self::LiveContract
        } else {
            Self::SingleFrame
        }
    }
}

impl From<bool> for TitleVisibility {
    fn from(include_titles: bool) -> Self {
        if include_titles {
            Self::Included
        } else {
            Self::Redacted
        }
    }
}

#[derive(Clone)]
struct WindowInfo {
    foreign_toplevel: ext_foreign_toplevel_handle_v1::ExtForeignToplevelHandleV1,
    cosmic_toplevel: Option<zcosmic_toplevel_handle_v1::ZcosmicToplevelHandleV1>,
    title: String,
    app_id: String,
    identifier: String,
    metadata_complete: bool,
}

impl WindowInfo {
    fn new(
        foreign_toplevel: ext_foreign_toplevel_handle_v1::ExtForeignToplevelHandleV1,
        cosmic_toplevel: Option<zcosmic_toplevel_handle_v1::ZcosmicToplevelHandleV1>,
    ) -> Self {
        Self {
            foreign_toplevel,
            cosmic_toplevel,
            title: String::new(),
            app_id: String::new(),
            identifier: String::new(),
            metadata_complete: false,
        }
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

impl ProvidesRegistryState for Probe {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }

    registry_handlers!(OutputState, SeatState);
}

impl CompositorHandler for Probe {
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

impl OutputHandler for Probe {
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

impl LayerShellHandler for Probe {
    fn closed(
        &mut self,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
        _layer: &LayerSurface,
    ) {
        self.fatal_error = Some("the compositor closed the keyboard overlay".to_owned());
    }

    fn configure(
        &mut self,
        _connection: &Connection,
        queue_handle: &QueueHandle<Self>,
        layer: &LayerSurface,
        _configure: LayerSurfaceConfigure,
        _serial: u32,
    ) {
        if self.overlay_pool.is_some() {
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
                self.overlay_pool = Some(pool);
            }
            Err(error) => {
                self.fatal_error = Some(format!("allocate transparent keyboard overlay: {error}"));
            }
        }
    }
}

impl SeatHandler for Probe {
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
                Err(error) => {
                    self.fatal_error = Some(format!("acquire keyboard input: {error}"));
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

impl KeyboardHandler for Probe {
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
        if self
            .layer
            .as_ref()
            .is_some_and(|layer| layer.wl_surface() == surface)
            && self.initial_hold_modifiers.is_none()
        {
            self.initial_hold_modifiers = Some(hold_modifiers_from_keysyms(keysyms));
            if let Err(error) = self.try_start_switching_session() {
                self.fatal_error = Some(error.to_string());
            }
        }
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
            self.handle_switching_event(SwitchingEvent::Tab);
        }
    }

    fn release_key(
        &mut self,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _serial: u32,
        event: KeyEvent,
    ) {
        if hold_modifier_from_keysym(event.keysym) != HoldModifiers::empty() {
            println!("Observed release of hold modifier {:?}", event.keysym);
        }
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
        self.handle_switching_event(SwitchingEvent::HoldModifiersChanged(
            hold_modifiers_from_state(modifiers),
        ));
    }
}

impl ShmHandler for Probe {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

impl ToplevelManagerHandler for Probe {
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

impl Dispatch<ext_foreign_toplevel_list_v1::ExtForeignToplevelListV1, GlobalData> for Probe {
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
                state
                    .windows
                    .push(WindowInfo::new(toplevel, Some(cosmic_toplevel)));
            }
            ext_foreign_toplevel_list_v1::Event::Finished => list.destroy(),
            _ => {}
        }
    }

    wayland_client::event_created_child!(Probe, ext_foreign_toplevel_list_v1::ExtForeignToplevelListV1, [
        ext_foreign_toplevel_list_v1::EVT_TOPLEVEL_OPCODE => (ext_foreign_toplevel_handle_v1::ExtForeignToplevelHandleV1, ())
    ]);
}

impl Dispatch<ext_foreign_toplevel_handle_v1::ExtForeignToplevelHandleV1, ()> for Probe {
    fn event(
        state: &mut Self,
        toplevel: &ext_foreign_toplevel_handle_v1::ExtForeignToplevelHandleV1,
        event: ext_foreign_toplevel_handle_v1::Event,
        _data: &(),
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
    ) {
        if let ext_foreign_toplevel_handle_v1::Event::Closed = event {
            if state.activation_target.as_ref().is_some_and(|target| {
                state.windows.iter().any(|window| {
                    window.foreign_toplevel == *toplevel && window.identifier == target.as_str()
                })
            }) {
                state.fatal_error =
                    Some("the selected Window closed before activation was confirmed".to_owned());
            }
            state
                .windows
                .retain(|window| window.foreign_toplevel != *toplevel);
            return;
        }

        let Some(window) = state
            .windows
            .iter_mut()
            .find(|window| window.foreign_toplevel == *toplevel)
        else {
            return;
        };
        match event {
            ext_foreign_toplevel_handle_v1::Event::Title { title } => window.title = title,
            ext_foreign_toplevel_handle_v1::Event::AppId { app_id } => window.app_id = app_id,
            ext_foreign_toplevel_handle_v1::Event::Identifier { identifier } => {
                window.identifier = identifier;
            }
            ext_foreign_toplevel_handle_v1::Event::Done => window.metadata_complete = true,
            ext_foreign_toplevel_handle_v1::Event::Closed => unreachable!(),
            _ => {}
        }
        if let Err(error) = state.try_start_switching_session() {
            state.fatal_error = Some(error.to_string());
        }
    }
}

impl Dispatch<zcosmic_toplevel_info_v1::ZcosmicToplevelInfoV1, GlobalData> for Probe {
    fn event(
        _state: &mut Self,
        _info: &zcosmic_toplevel_info_v1::ZcosmicToplevelInfoV1,
        _event: zcosmic_toplevel_info_v1::Event,
        _data: &GlobalData,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
    ) {
    }

    wayland_client::event_created_child!(Probe, zcosmic_toplevel_info_v1::ZcosmicToplevelInfoV1, [
        zcosmic_toplevel_info_v1::EVT_TOPLEVEL_OPCODE => (zcosmic_toplevel_handle_v1::ZcosmicToplevelHandleV1, GlobalData)
    ]);
}

impl Dispatch<zcosmic_toplevel_handle_v1::ZcosmicToplevelHandleV1, GlobalData> for Probe {
    fn event(
        state: &mut Self,
        toplevel: &zcosmic_toplevel_handle_v1::ZcosmicToplevelHandleV1,
        event: zcosmic_toplevel_handle_v1::Event,
        _data: &GlobalData,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
    ) {
        if let zcosmic_toplevel_handle_v1::Event::State { state: states } = event
            && toplevel_is_activated(&states)
        {
            let activated_window = state
                .windows
                .iter()
                .find(|window| window.cosmic_toplevel.as_ref() == Some(toplevel))
                .map(|window| WindowId::from(window.identifier.clone()));
            if activated_window.as_ref() == state.activation_target.as_ref() {
                let target = state
                    .activation_target
                    .take()
                    .expect("confirmed activation has a target");
                println!("Compositor confirmed activation of Window id={target}.");
                state.terminal_input = true;
            }
        }
        if let Err(error) = state.try_start_switching_session() {
            state.fatal_error = Some(error.to_string());
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

struct PendingCaptureRequest {
    window: WindowId,
    layout: ShmFrameLayout,
}

impl ShmCaptureHandler for Probe {
    fn constraints_ready(
        &mut self,
        queue_handle: &QueueHandle<Self>,
        session: &CaptureSession,
        constraints: ShmConstraints,
    ) {
        let window =
            ShmCaptureState::session_window(session).unwrap_or_else(|| WindowId::from("<unknown>"));
        let Some(layout) = constraints.negotiate() else {
            self.note_capture_failure(&format!(
                "Window {window} offered no supported exact-size four-byte SHM layout"
            ));
            self.capture_backend.stop_stream(&window);
            return;
        };
        if let Err(error) =
            self.capture_backend
                .request_frame(&window, layout, &self.shm, queue_handle)
        {
            self.note_capture_failure(&format!(
                "request initial SHM frame for Window {window} failed: {error:#}"
            ));
            self.capture_backend.stop_stream(&window);
        }
    }

    fn capture_stopped(&mut self, session: &CaptureSession) {
        let window =
            ShmCaptureState::session_window(session).unwrap_or_else(|| WindowId::from("<unknown>"));
        self.capture_backend.stop_stream(&window);
        self.note_capture_failure(&format!("capture stopped for Window {window}"));
    }

    fn frame_ready(&mut self, capture_frame: &CaptureFrame) {
        let completed = match self.capture_backend.complete_frame(capture_frame) {
            Ok(completed) => completed,
            Err(error) => {
                self.fatal_error = Some(format!("complete exact-size SHM frame: {error:#}"));
                return;
            }
        };
        let window = completed.window;
        let layout = completed.thumbnail.layout();
        let previous_frame_count = self.captured_frames.get(&window).copied().unwrap_or(0);
        if previous_frame_count > 0 && completed.damage.is_empty() {
            self.unchanged_windows.insert(window.clone());
            println!(
                "Suppressed unchanged SHM frame for Window {window}; no duplicate thumbnail was presented."
            );
            return;
        }
        self.capture_succeeded = true;
        let frame_count = self.captured_frames.entry(window.clone()).or_default();
        *frame_count += 1;
        println!(
            "Captured exact-size memory-only SHM frame for Window {}: {}x{}, {} bytes, {:?}.",
            window, layout.width, layout.height, layout.byte_len, layout.format
        );
        if self.capture_mode == CaptureProbeMode::LiveContract
            && *frame_count < LIVE_CONTRACT_FRAME_LIMIT
        {
            self.pending_recaptures
                .push(PendingCaptureRequest { window, layout });
        }
    }

    fn frame_failed(&mut self, capture_frame: &CaptureFrame) {
        let window = ShmCaptureState::frame_window(capture_frame)
            .unwrap_or_else(|| WindowId::from("<unknown>"));
        self.capture_backend.fail_frame(capture_frame);
        self.capture_backend.stop_stream(&window);
        self.note_capture_failure(&format!("SHM capture failed for Window {window}"));
    }
}

delegate_compositor!(Probe);
delegate_keyboard!(Probe);
delegate_layer!(Probe);
delegate_output!(Probe);
delegate_registry!(Probe);
delegate_seat!(Probe);
delegate_shm!(Probe);
wayland_client::delegate_dispatch!(Probe: [
    ext_image_copy_capture_manager_v1::ExtImageCopyCaptureManagerV1: GlobalData
] => ShmCaptureState);
wayland_client::delegate_dispatch!(Probe: [
    ext_foreign_toplevel_image_capture_source_manager_v1::ExtForeignToplevelImageCaptureSourceManagerV1: GlobalData
] => ShmCaptureState);
wayland_client::delegate_dispatch!(Probe: [
    ext_image_capture_source_v1::ExtImageCaptureSourceV1: GlobalData
] => ShmCaptureState);
wayland_client::delegate_dispatch!(Probe: [CaptureSession: CaptureSessionData] => ShmCaptureState);
wayland_client::delegate_dispatch!(Probe: [CaptureFrame: CaptureFrameData] => ShmCaptureState);
cosmic_client_toolkit::delegate_toplevel_manager!(Probe);
delegate_noop!(Probe: ignore wl_buffer::WlBuffer);
