//! Windows implementation (full read-write)

use anyhow::{bail, Context, Result};
use ipconfig::{get_adapters, IfType};
use std::{
    collections::HashMap,
    process::{Command, Stdio},
};

/* ------------------------------------------------------------------------- */
/* Data structure                                                            */
/* ------------------------------------------------------------------------- */

#[derive(Clone)]
pub struct NicInfo {
    pub name:        String,
    pub kind:        &'static str,
    pub mac:         Option<String>,
    pub ipv4_first:  Option<String>,
    pub gw_first:    Option<String>,
    pub dns_first:   Option<String>,
    pub enabled:     bool,
    pub oper_status: ipconfig::OperStatus,
}

/* ------------------------------------------------------------------------- */
/* Enumeration                                                               */
/* ------------------------------------------------------------------------- */

pub fn list_nics() -> Result<Vec<NicInfo>> {
    // 1. admin state for all interfaces
    let admin_map = query_all_admin_states()?; // <name, enabled>

    // 2. active adapters
    let mut active_map: HashMap<String, ipconfig::Adapter> = HashMap::new();
    for ad in get_adapters().context("GetAdaptersAddresses failed")? {
        active_map.insert(ad.friendly_name().to_string(), ad);
    }

    // 3. merge
    let mut out = Vec::<NicInfo>::new();
    for (name, enabled) in admin_map {
        if let Some(ad) = active_map.remove(&name) {
            out.push(build_from_adapter(&ad, enabled)?);
        } else {
            out.push(build_skeleton(&name, enabled));
        }
    }
    Ok(out)
}

/* ------------------------------------------------------------------------- */
/* Setters                                                                   */
/* ------------------------------------------------------------------------- */

pub fn apply_ip(name: &str, spec: &str) -> Result<()> {
    let v: Vec<_> = spec.split(',').map(|s| s.trim()).collect();
    if v.len() != 3 {
        bail!("format: ip,mask,gateway");
    }
    netsh(&[
        "interface", "ip", "set", "address",
        &format!("name=\"{name}\""),
        "static", v[0], v[1], v[2],
    ])
}

pub fn apply_dns(name: &str, list: &str) -> Result<()> {
    let addrs: Vec<_> = list
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();
    if addrs.is_empty() {
        bail!("need at least one DNS");
    }

    netsh(&["interface", "ip", "set", "dns", &format!("name=\"{name}\""), "dhcp"])?;

    netsh(&[
        "interface", "ip", "set", "dns", &format!("name=\"{name}\""),
        "static", addrs[0], "primary",
    ])?;

    for (idx, addr) in addrs.iter().enumerate().skip(1) {
        netsh(&[
            "interface", "ip", "add", "dns", &format!("name=\"{name}\""),
            addr, &format!("index={}", idx + 1),
        ])?;
    }
    Ok(())
}

pub fn set_enabled(name: &str, enabled: bool) -> Result<()> {
    let state = if enabled { "enabled" } else { "disabled" };
    netsh(&[
        "interface", "set", "interface",
        &format!("name=\"{name}\""),
        &format!("admin={state}"),
    ])
}

/* ------------------------------------------------------------------------- */
/* Helpers                                                                   */
/* ------------------------------------------------------------------------- */

fn netsh(args: &[&str]) -> Result<()> {
    let status = Command::new("netsh")
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;

    if !status.success() {
        bail!("netsh {:?} failed ({status})", args);
    }
    Ok(())
}

fn query_all_admin_states() -> Result<HashMap<String, bool>> {
    let out = Command::new("netsh")
        .args(["interface", "show", "interface"])
        .output()?;

    if !out.status.success() {
        bail!("netsh interface show interface failed");
    }

    let text = String::from_utf8_lossy(&out.stdout);
    let mut map = HashMap::<String, bool>::new();

    for line in text.lines().skip(3) {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('-') {
            continue;
        }
        let mut parts = trimmed.split_whitespace();
        let admin_raw = parts.next();
        let _state = parts.next();
        let _typ   = parts.next();
        let name_parts: Vec<_> = parts.collect();
        if admin_raw.is_none() || name_parts.is_empty() {
            continue;
        }
        let admin = admin_raw.unwrap().to_lowercase();
        let enabled = matches!(admin.as_str(), "enabled" | "已启用");
        map.insert(name_parts.join(" "), enabled);
    }
    Ok(map)
}

fn build_from_adapter(ad: &ipconfig::Adapter, enabled: bool) -> Result<NicInfo> {
    let kind = match ad.if_type() {
        IfType::Ieee80211      => "Wifi",
        IfType::EthernetCsmacd => "Wired",
        _                      => "Other",
    };

    let ipv4 = ad
        .ip_addresses()
        .iter()
        .find(|ip| ip.is_ipv4())
        .map(|ip| ip.to_string());

    let gw = ad.gateways().iter().next().map(|g| g.to_string());

    let dns_joined = ad
        .dns_servers()
        .iter()
        .map(|d| d.to_string())
        .collect::<Vec<_>>()
        .join(",");
    let dns_opt = if dns_joined.is_empty() { None } else { Some(dns_joined) };

    Ok(NicInfo {
        name: ad.friendly_name().to_string(),
        kind,
        mac: ad.physical_address().map(format_mac),
        ipv4_first: ipv4,
        gw_first: gw,
        dns_first: dns_opt,
        enabled,
        oper_status: ad.oper_status(),
    })
}

fn build_skeleton(name: &str, enabled: bool) -> NicInfo {
    let lname = name.to_ascii_lowercase();
    let kind = if lname.contains("wifi") || lname.contains("wireless") {
        "Wifi"
    } else if lname.contains("ethernet") || lname.contains("lan") {
        "Wired"
    } else {
        "Other"
    };

    NicInfo {
        name: name.to_string(),
        kind,
        mac: None,
        ipv4_first: None,
        gw_first: None,
        dns_first: None,
        enabled,
        oper_status: ipconfig::OperStatus::IfOperStatusDown,
    }
}

fn format_mac(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02X}", b)).collect::<Vec<_>>().join("-")
}

pub use ipconfig::OperStatus;   // single export used by UI
