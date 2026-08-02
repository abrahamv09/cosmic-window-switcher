// SPDX-License-Identifier: GPL-3.0-only

use std::{
    collections::VecDeque,
    sync::{Arc, Mutex, RwLock},
};

use anyhow::{Context, Result};
use rustix::{
    event::{EventfdFlags, eventfd},
    fd::{AsFd, BorrowedFd, OwnedFd},
};

use cosmic_window_switcher::{InvocationDirection, ServiceDiagnostics, SwitcherService};

mod accessibility;
mod diagnostics;
mod icons;
mod invocation;
mod overlay;
mod session_lifecycle;
mod window_observer;

type SharedService = Arc<RwLock<SwitcherService>>;
type PendingInvocations = Arc<WakeQueue<InvocationDirection>>;
type PendingLifecycleEvents = Arc<WakeQueue<cosmic_window_switcher::ServiceEvent>>;

struct WakeQueue<T> {
    values: Mutex<VecDeque<T>>,
    wake: OwnedFd,
}

impl<T> WakeQueue<T> {
    fn new() -> Result<Self> {
        let wake = eventfd(
            0,
            EventfdFlags::CLOEXEC | EventfdFlags::NONBLOCK | EventfdFlags::SEMAPHORE,
        )
        .context("create the service wake event")?;
        Ok(Self {
            values: Mutex::new(VecDeque::new()),
            wake,
        })
    }

    fn push(&self, value: T) {
        let mut values = self
            .values
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        values.push_back(value);
        let _wake_result = rustix::io::write(&self.wake, &1_u64.to_ne_bytes());
    }

    fn pop(&self) -> Option<T> {
        let mut values = self
            .values
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let value = values.pop_front();
        if value.is_some() {
            let mut value = [0_u8; size_of::<u64>()];
            let _wake_result = rustix::io::read(&self.wake, &mut value);
        }
        value
    }

    fn wake_fd(&self) -> BorrowedFd<'_> {
        self.wake.as_fd()
    }

    fn has_pending(&self) -> bool {
        let values = self
            .values
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        !values.is_empty()
    }
}

const BUS_NAME: &str = "io.github.abrahamv09.CosmicWindowSwitcher";
const OBJECT_PATH: &str = "/io/github/abrahamv09/CosmicWindowSwitcher";
const INTERFACE_NAME: &str = "io.github.abrahamv09.CosmicWindowSwitcher1";

pub fn run() -> Result<()> {
    crate::cosmic_session::verify("Switcher Service")?;

    let service = Arc::new(RwLock::new(SwitcherService::new()));
    let pending_invocations = Arc::new(WakeQueue::new()?);
    let pending_lifecycle_events = Arc::new(WakeQueue::new()?);
    let _lifecycle_monitor = session_lifecycle::monitor(Arc::clone(&pending_lifecycle_events))?;
    let _bus_connection =
        diagnostics::serve(Arc::clone(&service), Arc::clone(&pending_invocations))?;
    let mut window_observer = window_observer::WindowObserver::connect(
        Arc::clone(&service),
        pending_invocations,
        pending_lifecycle_events,
    )?;
    window_observer.synchronize_initial_windows()?;

    loop {
        if let Err(error) = window_observer.dispatch() {
            window_observer.compositor_lost();
            return Err(error);
        }
    }
}

pub fn status() -> Result<ServiceDiagnostics> {
    diagnostics::status()
}

pub fn invoke(direction: InvocationDirection) -> Result<()> {
    invocation::invoke(direction)
}
