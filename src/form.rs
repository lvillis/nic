#![allow(clippy::module_name_repetitions)]
//! Stand-alone mini forms (IPv4 / DNS) used in earlier prototypes.
//! Kept for reference; the current UI embeds a unified `Form` in `app.rs`.

use crate::platform::{self, NicInfo};
use anyhow::{bail, Result};
use crossterm::event::{KeyCode, KeyEvent};
use once_cell::sync::Lazy;
use regex::Regex;

static IP_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^(25[0-5]|2[0-4]\d|[01]?\d?\d)(\.(?!$)|$){4}$").unwrap());

fn valid_ip(s: &str) -> bool {
    IP_RE.is_match(s)
}

// -------------------------------------------------------------------------------------------------
// IPv4 editor
// -------------------------------------------------------------------------------------------------
#[derive(Clone)]
pub struct Ipv4Form {
    pub ip:   String,
    pub mask: String,
    pub gw:   String,
    /// 0 ip / 1 mask / 2 gw
    pub cursor: usize,
}

impl Ipv4Form {
    pub fn from_nic(n: &NicInfo) -> Self {
        Self {
            ip:   n.ipv4_first.clone().unwrap_or_default(),
            mask: "255.255.255.0".into(),
            gw:   String::new(),
            cursor: 0,
        }
    }

    /// Returns true when form content changed.
    pub fn on_key(&mut self, k: KeyEvent) -> bool {
        let field = match self.cursor {
            0 => &mut self.ip,
            1 => &mut self.mask,
            _ => &mut self.gw,
        };
        match k.code {
            KeyCode::Tab | KeyCode::Down => self.cursor = (self.cursor + 1) % 3,
            KeyCode::Up => self.cursor = (self.cursor + 2) % 3,
            KeyCode::Backspace => {
                field.pop();
                return true;
            }
            KeyCode::Char(c) if c.is_ascii_graphic() && field.len() < 18 => {
                field.push(c);
                return true;
            }
            _ => {}
        }
        false
    }

    pub fn apply(&self, nic: &NicInfo) -> Result<()> {
        if !(valid_ip(&self.ip) && valid_ip(&self.mask) && valid_ip(&self.gw)) {
            bail!("invalid IP/Mask/Gateway");
        }
        let spec = format!("{},{},{}", self.ip, self.mask, self.gw);
        platform::apply_ip(&nic.name, &spec)
    }
}

// -------------------------------------------------------------------------------------------------
// DNS editor
// -------------------------------------------------------------------------------------------------
#[derive(Clone)]
pub struct DnsForm {
    pub dns:    String,
    editing:    bool,
}

impl DnsForm {
    pub fn from_nic(n: &NicInfo) -> Self {
        Self {
            dns: n.dns_first.clone().unwrap_or_default(),
            editing: false,
        }
    }

    /// Returns true when content changed.
    pub fn on_key(&mut self, k: KeyEvent) -> bool {
        match k.code {
            KeyCode::Enter => self.editing = !self.editing,
            _ if !self.editing => {}
            KeyCode::Backspace => {
                self.dns.pop();
                return true;
            }
            KeyCode::Char(c) if c.is_ascii_graphic() && self.dns.len() < 40 => {
                self.dns.push(c);
                return true;
            }
            _ => {}
        }
        false
    }

    pub fn apply(&self, nic: &NicInfo) -> Result<()> {
        if self.dns.is_empty() {
            bail!("DNS empty");
        }
        platform::apply_dns(&nic.name, &self.dns)
    }
}
