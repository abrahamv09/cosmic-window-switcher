// SPDX-License-Identifier: GPL-3.0-only

use anyhow::{Result, bail};

pub fn verify(probe_name: &str) -> Result<()> {
    let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
    let session_type = std::env::var("XDG_SESSION_TYPE").unwrap_or_default();
    if !desktop
        .split(':')
        .any(|component| component.eq_ignore_ascii_case("COSMIC"))
        || session_type != "wayland"
    {
        bail!("the {probe_name} requires a COSMIC Wayland session");
    }
    Ok(())
}
