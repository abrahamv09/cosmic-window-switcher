// SPDX-License-Identifier: GPL-3.0-only

use std::{
    fmt::Write as _,
    sync::{Arc, RwLock},
};

use anyhow::{Context, Result};
use cosmic_client_toolkit::GlobalData;
use cosmic_protocols::toplevel_info::v1::client::{
    zcosmic_toplevel_handle_v1, zcosmic_toplevel_info_v1,
};
use smithay_client_toolkit::{
    delegate_registry,
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
};
use wayland_client::{Connection, Dispatch, QueueHandle, globals::registry_queue_init};
use wayland_protocols::ext::foreign_toplevel_list::v1::client::{
    ext_foreign_toplevel_handle_v1, ext_foreign_toplevel_list_v1,
};
use zbus::blocking::{Connection as BusConnection, Proxy as BusProxy, connection};

use cosmic_window_switcher::{
    HistoryAccuracy, ServiceDiagnostics, SwitcherService, WindowEvent, WindowId,
};

const BUS_NAME: &str = "io.github.abrahamv09.CosmicWindowSwitcher";
const OBJECT_PATH: &str = "/io/github/abrahamv09/CosmicWindowSwitcher";
const INTERFACE_NAME: &str = "io.github.abrahamv09.CosmicWindowSwitcher1";

type SharedService = Arc<RwLock<SwitcherService>>;

pub fn run() -> Result<()> {
    crate::cosmic_session::verify("Switcher Service")?;

    let connection = Connection::connect_to_env().context("connect to the Wayland compositor")?;
    let (globals, mut event_queue) =
        registry_queue_init(&connection).context("read Wayland globals")?;
    let queue_handle = event_queue.handle();
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

    let shared_service = Arc::new(RwLock::new(SwitcherService::new()));
    let mut observer = ServiceObserver {
        registry_state,
        _foreign_toplevel_list: foreign_toplevel_list,
        cosmic_toplevel_info,
        windows: Vec::new(),
        service: Arc::clone(&shared_service),
    };

    event_queue
        .roundtrip(&mut observer)
        .context("receive initial COSMIC Window state")?;
    event_queue
        .roundtrip(&mut observer)
        .context("finish initial COSMIC Window state")?;
    event_queue
        .roundtrip(&mut observer)
        .context("synchronize initial COSMIC Window state")?;
    observer.complete_initial_discovery();

    let _bus_connection = connection::Builder::session()
        .context("connect to the user-session D-Bus")?
        .serve_at(
            OBJECT_PATH,
            DiagnosticsInterface {
                service: shared_service,
            },
        )
        .context("register the Switcher Service D-Bus interface")?
        .name(BUS_NAME)
        .context("request the single Switcher Service D-Bus name")?
        .build()
        .context("start the Switcher Service D-Bus connection")?;

    loop {
        event_queue
            .blocking_dispatch(&mut observer)
            .context("observe COSMIC Window events")?;
        connection
            .flush()
            .context("flush the COSMIC compositor connection")?;
    }
}

pub fn status() -> Result<String> {
    let connection =
        BusConnection::session().context("connect to the user-session D-Bus for status")?;
    let proxy = BusProxy::new(&connection, BUS_NAME, OBJECT_PATH, INTERFACE_NAME)
        .context("create the Switcher Service status proxy")?;
    let (warm_up, mru_order): (bool, Vec<String>) = proxy
        .call("Status", &())
        .context("request Switcher Service status")?;

    Ok(format_status(warm_up, &mru_order))
}

fn format_status(warm_up: bool, mru_order: &[String]) -> String {
    let history = if warm_up { "warm-up" } else { "accurate" };
    let mut output = format!(
        "service: running\nmru_history: {history}\nwindow_count: {}",
        mru_order.len()
    );
    if !mru_order.is_empty() {
        output.push_str("\nmru_order:");
        for (position, id) in mru_order.iter().enumerate() {
            write!(output, "\n  {}. {id}", position + 1)
                .expect("writing diagnostics to a String cannot fail");
        }
    }
    output
}

struct DiagnosticsInterface {
    service: SharedService,
}

#[zbus::interface(name = "io.github.abrahamv09.CosmicWindowSwitcher1")]
impl DiagnosticsInterface {
    fn status(&self) -> (bool, Vec<String>) {
        let diagnostics = read_diagnostics(&self.service);
        (
            diagnostics.history == HistoryAccuracy::WarmUp,
            diagnostics
                .mru_order
                .iter()
                .map(|id| id.as_str().to_owned())
                .collect(),
        )
    }
}

fn read_diagnostics(service: &RwLock<SwitcherService>) -> ServiceDiagnostics {
    service
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .diagnostics()
}

fn observe(service: &RwLock<SwitcherService>, event: WindowEvent) {
    service
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .observe(event);
}

struct ObservedWindow {
    foreign_toplevel: ext_foreign_toplevel_handle_v1::ExtForeignToplevelHandleV1,
    cosmic_toplevel: zcosmic_toplevel_handle_v1::ZcosmicToplevelHandleV1,
    id: Option<WindowId>,
    activated: bool,
    registered: bool,
}

struct ServiceObserver {
    registry_state: RegistryState,
    _foreign_toplevel_list: ext_foreign_toplevel_list_v1::ExtForeignToplevelListV1,
    cosmic_toplevel_info: zcosmic_toplevel_info_v1::ZcosmicToplevelInfoV1,
    windows: Vec<ObservedWindow>,
    service: SharedService,
}

impl ServiceObserver {
    fn complete_initial_discovery(&self) {
        self.service
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .complete_initial_discovery();
    }

    fn register_ready_windows(&mut self) {
        let mut events = Vec::new();
        for window in &mut self.windows {
            if window.registered {
                continue;
            }
            let Some(id) = window.id.clone() else {
                break;
            };
            window.registered = true;
            events.push(WindowEvent::Discovered(id.clone()));
            if window.activated {
                events.push(WindowEvent::Activated(id));
            }
        }
        for event in events {
            observe(&self.service, event);
        }
    }
}

impl ProvidesRegistryState for ServiceObserver {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }

    registry_handlers!();
}

impl Dispatch<ext_foreign_toplevel_list_v1::ExtForeignToplevelListV1, GlobalData>
    for ServiceObserver
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
                state.windows.push(ObservedWindow {
                    foreign_toplevel: toplevel,
                    cosmic_toplevel,
                    id: None,
                    activated: false,
                    registered: false,
                });
            }
            ext_foreign_toplevel_list_v1::Event::Finished => list.destroy(),
            _ => {}
        }
    }

    wayland_client::event_created_child!(ServiceObserver, ext_foreign_toplevel_list_v1::ExtForeignToplevelListV1, [
        ext_foreign_toplevel_list_v1::EVT_TOPLEVEL_OPCODE => (ext_foreign_toplevel_handle_v1::ExtForeignToplevelHandleV1, ())
    ]);
}

impl Dispatch<ext_foreign_toplevel_handle_v1::ExtForeignToplevelHandleV1, ()> for ServiceObserver {
    fn event(
        state: &mut Self,
        toplevel: &ext_foreign_toplevel_handle_v1::ExtForeignToplevelHandleV1,
        event: ext_foreign_toplevel_handle_v1::Event,
        _data: &(),
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
    ) {
        if let ext_foreign_toplevel_handle_v1::Event::Closed = event {
            if let Some(position) = state
                .windows
                .iter()
                .position(|window| window.foreign_toplevel == *toplevel)
            {
                let window = state.windows.remove(position);
                if window.registered
                    && let Some(id) = window.id
                {
                    observe(&state.service, WindowEvent::Closed(id));
                }
                state.register_ready_windows();
            }
            return;
        }

        let Some(window) = state
            .windows
            .iter_mut()
            .find(|window| window.foreign_toplevel == *toplevel)
        else {
            return;
        };
        if let ext_foreign_toplevel_handle_v1::Event::Identifier { identifier } = event {
            window.id = Some(WindowId::from(identifier));
            state.register_ready_windows();
        }
    }
}

impl Dispatch<zcosmic_toplevel_info_v1::ZcosmicToplevelInfoV1, GlobalData> for ServiceObserver {
    fn event(
        _state: &mut Self,
        _info: &zcosmic_toplevel_info_v1::ZcosmicToplevelInfoV1,
        _event: zcosmic_toplevel_info_v1::Event,
        _data: &GlobalData,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
    ) {
    }

    wayland_client::event_created_child!(ServiceObserver, zcosmic_toplevel_info_v1::ZcosmicToplevelInfoV1, [
        zcosmic_toplevel_info_v1::EVT_TOPLEVEL_OPCODE => (zcosmic_toplevel_handle_v1::ZcosmicToplevelHandleV1, GlobalData)
    ]);
}

impl Dispatch<zcosmic_toplevel_handle_v1::ZcosmicToplevelHandleV1, GlobalData> for ServiceObserver {
    fn event(
        state: &mut Self,
        toplevel: &zcosmic_toplevel_handle_v1::ZcosmicToplevelHandleV1,
        event: zcosmic_toplevel_handle_v1::Event,
        _data: &GlobalData,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
    ) {
        let zcosmic_toplevel_handle_v1::Event::State { state: states } = event else {
            return;
        };
        let Some(window) = state
            .windows
            .iter_mut()
            .find(|window| window.cosmic_toplevel == *toplevel)
        else {
            return;
        };

        let activated = toplevel_is_activated(&states);
        let became_active = activated && !window.activated;
        window.activated = activated;
        let activated_id = became_active
            .then(|| window.id.clone())
            .flatten()
            .filter(|_| window.registered);
        if let Some(id) = activated_id {
            observe(&state.service, WindowEvent::Activated(id));
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

delegate_registry!(ServiceObserver);

#[cfg(test)]
mod tests {
    use super::format_status;

    #[test]
    fn status_output_identifies_warm_up_without_window_titles() {
        let output = format_status(true, &["opaque-a".to_owned(), "opaque-b".to_owned()]);

        assert_eq!(
            output,
            "service: running\nmru_history: warm-up\nwindow_count: 2\nmru_order:\n  1. opaque-a\n  2. opaque-b"
        );
    }
}
