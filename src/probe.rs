// SPDX-License-Identifier: GPL-3.0-only

use std::sync::Mutex;

use anyhow::{Context, Result, bail};
use cosmic_client_toolkit::{
    GlobalData,
    screencopy::{
        CaptureFrame, CaptureOptions, CaptureSession, CaptureSource, FailureReason, Formats,
        ScreencopyFrameData, ScreencopyFrameDataExt, ScreencopyHandler, ScreencopySessionData,
        ScreencopySessionDataExt, ScreencopyState,
    },
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
    Connection, Dispatch, QueueHandle, WEnum, delegate_noop,
    globals::registry_queue_init,
    protocol::{wl_buffer, wl_keyboard, wl_output, wl_seat, wl_shm, wl_surface},
};
use wayland_protocols::ext::foreign_toplevel_list::v1::client::{
    ext_foreign_toplevel_handle_v1, ext_foreign_toplevel_list_v1,
};

use cosmic_window_switcher::{
    APPLICATION_ID, HoldModifiers, SessionEffect, SwitchingEvent, SwitchingSession, WindowId,
};

pub fn run(include_titles: bool) -> Result<()> {
    verify_cosmic_session()?;

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
        screencopy_state: ScreencopyState::new(&globals, &queue_handle),
        toplevel_manager,
        layer: None,
        overlay_pool: None,
        keyboard: None,
        seat: None,
        session: None,
        initial_hold_modifiers: None,
        pending_switching_events: Vec::new(),
        capture_sessions: Vec::new(),
        capture_attempts: 0,
        capture_failures: 0,
        capture_succeeded: false,
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

    while !probe.is_finished() {
        event_queue
            .blocking_dispatch(&mut probe)
            .context("dispatch COSMIC probe event")?;
        connection
            .flush()
            .context("flush requests to the COSMIC compositor")?;
    }

    probe.into_result()
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

fn verify_cosmic_session() -> Result<()> {
    let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
    let session_type = std::env::var("XDG_SESSION_TYPE").unwrap_or_default();
    if !desktop
        .split(':')
        .any(|component| component.eq_ignore_ascii_case("COSMIC"))
        || session_type != "wayland"
    {
        bail!("the probe requires a COSMIC Wayland session");
    }
    Ok(())
}

struct Probe {
    registry_state: RegistryState,
    output_state: OutputState,
    seat_state: SeatState,
    shm: Shm,
    cosmic_toplevel_info: zcosmic_toplevel_info_v1::ZcosmicToplevelInfoV1,
    windows: Vec<WindowInfo>,
    screencopy_state: ScreencopyState,
    toplevel_manager: ToplevelManagerState,
    layer: Option<LayerSurface>,
    overlay_pool: Option<RawPool>,
    keyboard: Option<wl_keyboard::WlKeyboard>,
    seat: Option<wl_seat::WlSeat>,
    session: Option<SwitchingSession>,
    initial_hold_modifiers: Option<HoldModifiers>,
    pending_switching_events: Vec<SwitchingEvent>,
    capture_sessions: Vec<CaptureSession>,
    capture_attempts: usize,
    capture_failures: usize,
    capture_succeeded: bool,
    terminal_input: bool,
    management_can_activate: bool,
    title_visibility: TitleVisibility,
    activation_target: Option<WindowId>,
    activation_requests: usize,
    fatal_error: Option<String>,
}

impl Probe {
    fn into_result(self) -> Result<()> {
        if let Some(message) = self.fatal_error {
            bail!("{message}");
        }
        if !self.capture_succeeded {
            bail!(
                "no SHM frame was captured ({} attempt(s), {} failure(s)); all Windows remained discoverable",
                self.capture_attempts,
                self.capture_failures
            );
        }
        Ok(())
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
            let capture_session = self
                .screencopy_state
                .capturer()
                .create_session(
                    &CaptureSource::Toplevel(window.foreign_toplevel.clone()),
                    CaptureOptions::empty(),
                    queue_handle,
                    CaptureData {
                        data: ScreencopySessionData::default(),
                        window_id: WindowId::from(window.identifier.clone()),
                    },
                )
                .with_context(|| {
                    format!("create capture session for Window {}", window.identifier)
                })?;
            self.capture_sessions.push(capture_session);
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
            SwitchingSession::new(windows, initial_hold_modifiers)
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

struct CaptureData {
    data: ScreencopySessionData,
    window_id: WindowId,
}

impl ScreencopySessionDataExt for CaptureData {
    fn screencopy_session_data(&self) -> &ScreencopySessionData {
        &self.data
    }
}

struct FrameData {
    data: ScreencopyFrameData,
    window_id: WindowId,
    pool: Mutex<RawPool>,
    size: (u32, u32),
    format: wl_shm::Format,
}

impl ScreencopyFrameDataExt for FrameData {
    fn screencopy_frame_data(&self) -> &ScreencopyFrameData {
        &self.data
    }
}

fn capture_session_window_id(session: &CaptureSession) -> WindowId {
    session.data::<CaptureData>().map_or_else(
        || WindowId::from("<unknown>"),
        |data| data.window_id.clone(),
    )
}

fn capture_frame_window_id(frame: &CaptureFrame) -> WindowId {
    frame.data::<FrameData>().map_or_else(
        || WindowId::from("<unknown>"),
        |data| data.window_id.clone(),
    )
}

impl ScreencopyHandler for Probe {
    fn screencopy_state(&mut self) -> &mut ScreencopyState {
        &mut self.screencopy_state
    }

    fn init_done(
        &mut self,
        _connection: &Connection,
        queue_handle: &QueueHandle<Self>,
        session: &CaptureSession,
        formats: &Formats,
    ) {
        let Some(format) = preferred_shm_format(&formats.shm_formats) else {
            let window_id = capture_session_window_id(session);
            self.note_capture_failure(&format!(
                "Window {window_id} offered no supported four-byte SHM format"
            ));
            return;
        };

        let (width, height) = formats.buffer_size;
        let Some(byte_len) = exact_frame_byte_len(width, height) else {
            self.note_capture_failure(&format!(
                "Window capture reported invalid size {width}x{height}"
            ));
            return;
        };

        let mut pool = match RawPool::new(byte_len, &self.shm) {
            Ok(pool) => pool,
            Err(error) => {
                self.note_capture_failure(&format!("allocate memory-only SHM frame: {error}"));
                return;
            }
        };
        let Ok(width_i32) = i32::try_from(width) else {
            self.note_capture_failure(&format!("Window capture width {width} is too large"));
            return;
        };
        let Ok(height_i32) = i32::try_from(height) else {
            self.note_capture_failure(&format!("Window capture height {height} is too large"));
            return;
        };
        let Some(stride) = width_i32.checked_mul(4) else {
            self.note_capture_failure(&format!(
                "Window capture width {width} overflows its stride"
            ));
            return;
        };
        let window_id = capture_session_window_id(session);
        let buffer = pool.create_buffer(0, width_i32, height_i32, stride, format, (), queue_handle);
        session.capture(
            &buffer,
            &[],
            queue_handle,
            FrameData {
                data: ScreencopyFrameData::default(),
                window_id,
                pool: Mutex::new(pool),
                size: formats.buffer_size,
                format,
            },
        );
    }

    fn stopped(
        &mut self,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
        session: &CaptureSession,
    ) {
        let window_id = capture_session_window_id(session);
        self.note_capture_failure(&format!("capture stopped for Window {window_id}"));
    }

    fn ready(
        &mut self,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
        capture_frame: &CaptureFrame,
        _frame: cosmic_client_toolkit::screencopy::Frame,
    ) {
        let Some(data) = capture_frame.data::<FrameData>() else {
            self.fatal_error = Some("capture completed without frame metadata".to_owned());
            return;
        };
        let pool = data.pool.lock().expect("capture pool lock is not poisoned");
        let expected = exact_frame_byte_len(data.size.0, data.size.1);
        if expected != Some(pool.len()) {
            self.fatal_error = Some(format!(
                "Window {} returned a non-exact SHM frame",
                data.window_id
            ));
            return;
        }
        self.capture_succeeded = true;
        println!(
            "Captured exact-size memory-only SHM frame for Window {}: {}x{}, {} bytes, {:?}.",
            data.window_id,
            data.size.0,
            data.size.1,
            pool.len(),
            data.format
        );
    }

    fn failed(
        &mut self,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
        capture_frame: &CaptureFrame,
        reason: WEnum<FailureReason>,
    ) {
        let window_id = capture_frame_window_id(capture_frame);
        self.note_capture_failure(&format!(
            "SHM capture failed for Window {window_id}: {reason:?}"
        ));
    }
}

fn preferred_shm_format(formats: &[wl_shm::Format]) -> Option<wl_shm::Format> {
    [
        wl_shm::Format::Abgr8888,
        wl_shm::Format::Argb8888,
        wl_shm::Format::Xbgr8888,
        wl_shm::Format::Xrgb8888,
    ]
    .into_iter()
    .find(|candidate| formats.contains(candidate))
}

fn exact_frame_byte_len(width: u32, height: u32) -> Option<usize> {
    usize::try_from(width)
        .ok()?
        .checked_mul(usize::try_from(height).ok()?)?
        .checked_mul(4)
}

delegate_compositor!(Probe);
delegate_keyboard!(Probe);
delegate_layer!(Probe);
delegate_output!(Probe);
delegate_registry!(Probe);
delegate_seat!(Probe);
delegate_shm!(Probe);
cosmic_client_toolkit::delegate_screencopy!(Probe);
cosmic_client_toolkit::delegate_toplevel_manager!(Probe);
delegate_noop!(Probe: ignore wl_buffer::WlBuffer);
