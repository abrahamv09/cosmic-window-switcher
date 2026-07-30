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
mod window_observer;

type SharedService = Arc<RwLock<SwitcherService>>;
type PendingInvocations = Arc<InvocationQueue>;

struct InvocationQueue {
    directions: Mutex<VecDeque<InvocationDirection>>,
    wake: OwnedFd,
}

impl InvocationQueue {
    fn new() -> Result<Self> {
        let wake = eventfd(
            0,
            EventfdFlags::CLOEXEC | EventfdFlags::NONBLOCK | EventfdFlags::SEMAPHORE,
        )
        .context("create the invocation wake event")?;
        Ok(Self {
            directions: Mutex::new(VecDeque::new()),
            wake,
        })
    }

    fn push(&self, direction: InvocationDirection) {
        let mut directions = self
            .directions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        directions.push_back(direction);
        let _wake_result = rustix::io::write(&self.wake, &1_u64.to_ne_bytes());
    }

    fn pop(&self) -> Option<InvocationDirection> {
        let mut directions = self
            .directions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let direction = directions.pop_front();
        if direction.is_some() {
            let mut value = [0_u8; size_of::<u64>()];
            let _wake_result = rustix::io::read(&self.wake, &mut value);
        }
        direction
    }

    fn wake_fd(&self) -> BorrowedFd<'_> {
        self.wake.as_fd()
    }
}

const BUS_NAME: &str = "io.github.abrahamv09.CosmicWindowSwitcher";
const OBJECT_PATH: &str = "/io/github/abrahamv09/CosmicWindowSwitcher";
const INTERFACE_NAME: &str = "io.github.abrahamv09.CosmicWindowSwitcher1";

pub fn run() -> Result<()> {
    crate::cosmic_session::verify("Switcher Service")?;

    let service = Arc::new(RwLock::new(SwitcherService::new()));
    let pending_invocations = Arc::new(InvocationQueue::new()?);
    let _bus_connection =
        diagnostics::serve(Arc::clone(&service), Arc::clone(&pending_invocations))?;
    let mut window_observer =
        window_observer::WindowObserver::connect(Arc::clone(&service), pending_invocations)?;
    window_observer.synchronize_initial_windows()?;

    loop {
        window_observer.dispatch()?;
    }
}

pub fn status() -> Result<ServiceDiagnostics> {
    diagnostics::status()
}

pub fn invoke(direction: InvocationDirection) -> Result<()> {
    invocation::invoke(direction)
}
