#![allow(clippy::module_name_repetitions)]
//! Unix implementation (Linux/macOS).
//!
//! Read-only: shows NIC info but does not modify settings.

use anyhow::{bail, Result};
use pnet_datalink;

/// Minimal NIC data for the UI.
#[derive(Clone)]
pub struct NicInfo {
    pub name:       String,
    pub kind:       &'static str,
    pub mac:        Option<String>,
    pub ipv4_first: Option<String>,
    pub gw_first:   Option<String>,
    pub dns_first:  Option<String>,
    pub enabled:    bool, // always true on Unix (display-only)
}

pub fn list_nics() -> Result<Vec<NicInfo>> {
    let mut list = Vec::new();
    for iface in pnet_datalink::interfaces() {
        if iface.is_loopback() {
            continue;
        }
        let kind = if iface.name.starts_with("wl") { "Wifi" } else { "Wired" };
        let ipv4 = iface
            .ips
            .iter()
            .find(|ip| ip.ip().is_ipv4())
            .map(|ip| ip.ip().to_string());
        list.push(NicInfo {
            name: iface.name.clone(),
            kind,
            mac: iface.mac.map(|m| format!(
                "{:02X}-{:02X}-{:02X}-{:02X}-{:02X}-{:02X}",
                m.0, m.1, m.2, m.3, m.4, m.5
            )),
            ipv4_first: ipv4,
            gw_first: None,
            dns_first: None,
            enabled: true,
        });
    }
    Ok(list)
}

// setters are not supported on Unix targets
pub fn apply_ip(_: &str, _: &str)  -> Result<()> { bail!("not supported on this platform") }
pub fn apply_dns(_: &str, _: &str) -> Result<()> { bail!("not supported on this platform") }
pub fn set_enabled(_: &str, _: bool) -> Result<()> {
    bail!("enable/disable NIC not supported on this platform")
}
