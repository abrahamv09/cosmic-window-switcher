// SPDX-License-Identifier: GPL-3.0-only

use anyhow::{Result, bail};

pub fn verify(probe_name: &str) -> Result<()> {
    if !is_compatible() {
        bail!("the {probe_name} requires a COSMIC Wayland session");
    }
    Ok(())
}

pub fn is_compatible() -> bool {
    let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
    let session_type = std::env::var("XDG_SESSION_TYPE").unwrap_or_default();
    desktop
        .split(':')
        .any(|component| component.eq_ignore_ascii_case("COSMIC"))
        && session_type == "wayland"
}
