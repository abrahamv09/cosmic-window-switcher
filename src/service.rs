// SPDX-License-Identifier: GPL-3.0-only

use std::sync::{Arc, RwLock};

use anyhow::Result;

use cosmic_window_switcher::{ServiceDiagnostics, SwitcherService};

mod diagnostics;
mod window_observer;

type SharedService = Arc<RwLock<SwitcherService>>;

pub fn run() -> Result<()> {
    crate::cosmic_session::verify("Switcher Service")?;

    let service = Arc::new(RwLock::new(SwitcherService::new()));
    let _bus_connection = diagnostics::serve(Arc::clone(&service))?;
    let mut window_observer = window_observer::WindowObserver::connect(Arc::clone(&service))?;
    window_observer.synchronize_initial_windows()?;

    loop {
        window_observer.dispatch()?;
    }
}

pub fn status() -> Result<ServiceDiagnostics> {
    diagnostics::status()
}
