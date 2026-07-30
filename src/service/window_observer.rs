// SPDX-License-Identifier: GPL-3.0-only

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
use wayland_client::{Connection, Dispatch, EventQueue, QueueHandle, globals::registry_queue_init};
use wayland_protocols::ext::foreign_toplevel_list::v1::client::{
    ext_foreign_toplevel_handle_v1, ext_foreign_toplevel_list_v1,
};

use cosmic_window_switcher::{WindowEvent, WindowId};

use super::SharedService;

pub(super) struct WindowObserver {
    connection: Connection,
    event_queue: EventQueue<ProtocolObserver>,
    state: ProtocolObserver,
}

impl WindowObserver {
    pub(super) fn connect(service: SharedService) -> Result<Self> {
        let connection =
            Connection::connect_to_env().context("connect to the Wayland compositor")?;
        let (globals, event_queue) =
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
        let state = ProtocolObserver {
            registry_state,
            _foreign_toplevel_list: foreign_toplevel_list,
            cosmic_toplevel_info,
            windows: Vec::new(),
            observations: ObservationLedger::default(),
            next_observation_key: 0,
            service,
        };

        Ok(Self {
            connection,
            event_queue,
            state,
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
        self.event_queue
            .blocking_dispatch(&mut self.state)
            .context("observe COSMIC Window events")?;
        self.connection
            .flush()
            .context("flush the COSMIC compositor connection")
    }
}

#[derive(Clone)]
struct ObservedWindow {
    key: ObservationKey,
    foreign_toplevel: ext_foreign_toplevel_handle_v1::ExtForeignToplevelHandleV1,
    cosmic_toplevel: zcosmic_toplevel_handle_v1::ZcosmicToplevelHandleV1,
}

struct ProtocolObserver {
    registry_state: RegistryState,
    _foreign_toplevel_list: ext_foreign_toplevel_list_v1::ExtForeignToplevelListV1,
    cosmic_toplevel_info: zcosmic_toplevel_info_v1::ZcosmicToplevelInfoV1,
    windows: Vec<ObservedWindow>,
    observations: ObservationLedger,
    next_observation_key: u64,
    service: SharedService,
}

impl ProtocolObserver {
    fn apply(&mut self, event: Observation) {
        let window_events = self.observations.apply(event);
        let mut service = self
            .service
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for window_event in window_events {
            service.observe(window_event);
        }
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

    registry_handlers!();
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
        let zcosmic_toplevel_handle_v1::Event::State { state: states } = event else {
            return;
        };
        let Some(key) = state
            .windows
            .iter()
            .find(|window| window.cosmic_toplevel == *toplevel)
            .map(|window| window.key)
        else {
            return;
        };
        state.apply(Observation::ActivationChanged {
            key,
            activated: toplevel_is_activated(&states),
        });
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

delegate_registry!(ProtocolObserver);

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
