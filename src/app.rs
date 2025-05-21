#![allow(clippy::too_many_lines)]
//! Runtime state and domain logic of the NIC configurator.

use crate::platform::{self, set_enabled, NicInfo};
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};
use crossbeam_channel::Sender;
use std::{
    thread,
    time::{Duration, Instant},
};

// -------------------------------------------------------------------------------------------------
// Spinner frames & timing constants
// -------------------------------------------------------------------------------------------------
pub const SPIN_FRAMES: [&str; 10] =
    ["⠋", "⠙", "⠚", "⠞", "⠖", "⠦", "⠴", "⠲", "⠳", "⠓"];

const MSG_HOLD: Duration = Duration::from_secs(2);
const BUSY_MIN: Duration = Duration::from_millis(500);

// -------------------------------------------------------------------------------------------------
// Form data model
// -------------------------------------------------------------------------------------------------
#[derive(Clone, Default)]
pub struct Form {
    pub ip:      String,
    pub mask:    String,
    pub gw:      String,
    pub dns:     String,
    pub enabled: bool,
    /// Cursor index in the form (0–4)
    pub cursor:  usize,
}

// -------------------------------------------------------------------------------------------------
// Focused UI region
// -------------------------------------------------------------------------------------------------
#[derive(PartialEq)]
pub enum Focus {
    List,
    Form,
    Filter,
}

// -------------------------------------------------------------------------------------------------
// Application-wide state container
// -------------------------------------------------------------------------------------------------
pub struct App {
    // ------ list view ------
    pub list:   Vec<NicInfo>,
    pub select: usize,
    pub filter: String,

    // ------ form ------
    pub form:  Form,
    pub focus: Focus,
    pub dirty: bool,

    // ------ pop-ups / busy / help ------
    pub confirm_save: bool,
    pub busy:         bool,
    pub busy_since:   Instant,
    pub spin:         usize,
    pub show_help:    bool,

    // ------ toast message ------
    pub message: Option<(String, Instant)>,
}

// =================================================================================================
// Initialization & refresh helpers
// =================================================================================================

/// Returns NIC list sorted by name for deterministic ordering.
fn list_sorted() -> Result<Vec<NicInfo>> {
    let mut v = platform::list_nics()?;
    v.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(v)
}

impl App {
    /// Build initial state.
    pub fn init() -> Result<Self> {
        let mut s = Self {
            list: list_sorted()?,
            select: 0,
            filter: String::new(),
            form: Form::default(),
            focus: Focus::List,
            dirty: false,
            confirm_save: false,
            busy: false,
            busy_since: Instant::now(),
            spin: 0,
            show_help: false,
            message: None,
        };
        s.load_sel();
        Ok(s)
    }

    /// Refresh NIC list while attempting to keep current selection.
    pub fn refresh_keep(&mut self) -> Result<()> {
        let sel_name = self.current_nic().map(|n| n.name.clone());
        self.list = list_sorted()?;
        self.select = sel_name
            .and_then(|n| self.list.iter().position(|x| x.name == n))
            .unwrap_or(0);
        self.load_sel();
        Ok(())
    }

    /// Force complete reload and reset selection to first row.
    pub fn refresh(&mut self) -> Result<()> {
        self.list = list_sorted()?;
        self.select = 0;
        self.load_sel();
        Ok(())
    }
}

// =================================================================================================
// Convenience helpers
// =================================================================================================

impl App {
    /// List filtered by current keyword.
    pub fn filtered(&self) -> Vec<&NicInfo> {
        if self.filter.is_empty() {
            self.list.iter().collect()
        } else {
            let q = self.filter.to_lowercase();
            self.list
                .iter()
                .filter(|n| n.name.to_lowercase().contains(&q))
                .collect()
        }
    }

    pub fn current_nic(&self) -> Option<&NicInfo> {
        self.filtered().get(self.select).copied()
    }

    /// Indicates whether quit is currently allowed.
    pub fn allow_quit(&self) -> bool {
        !self.dirty && !self.busy && !self.confirm_save && !self.show_help
    }

    /// Push a transient toast message.
    pub fn toast(&mut self, s: &str) {
        self.message = Some((s.into(), Instant::now()));
        self.dirty = false;
    }

    /// Clear toast when timeout expires.
    pub fn expire_toast(&mut self) {
        if let Some((_, t)) = &self.message {
            if t.elapsed() > MSG_HOLD {
                self.message = None;
            }
        }
    }

    /// Load selected NIC details into the form.
    fn load_sel(&mut self) {
        if let Some(n) = self.current_nic() {
            self.form = Form {
                ip:      n.ipv4_first.clone().unwrap_or_default(),
                mask:    "255.255.255.0".into(),
                gw:      n.gw_first.clone().unwrap_or_default(),
                dns:     n.dns_first.clone().unwrap_or_default(),
                enabled: n.enabled,
                cursor:  0,
            };
            self.dirty = false;
        }
    }
}

// =================================================================================================
// Keyboard handling (list / filter / form)
// =================================================================================================

impl App {
    pub fn on_key(&mut self, k: KeyEvent) -> Result<()> {
        // 1) help popup
        if self.show_help {
            match k.code {
                KeyCode::F(1) | KeyCode::Char('?') | KeyCode::Esc => self.show_help = false,
                _ => {}
            }
            return Ok(());
        }

        // 2) save-confirmation popup
        if self.confirm_save {
            match k.code {
                KeyCode::Enter => {
                    self.confirm_save = false;
                    return Ok(());
                }
                KeyCode::Esc => self.confirm_save = false,
                _ => {}
            }
            return Ok(());
        }

        // 3) busy state (form input disabled)
        if self.busy && self.focus == Focus::Form {
            return Ok(());
        }

        // 4) global shortcuts
        if matches!(k.code, KeyCode::F(1) | KeyCode::Char('?')) {
            self.show_help = true;
            return Ok(());
        }

        // 5) dispatch by focus
        match self.focus {
            Focus::List   => self.key_list(k),
            Focus::Filter => self.key_filter(k),
            Focus::Form   => self.key_form(k),
        }
    }

    // ---- list view keys ----
    fn key_list(&mut self, k: KeyEvent) -> Result<()> {
        match k.code {
            KeyCode::Up if self.select > 0 => {
                self.select -= 1;
                self.load_sel();
            }
            KeyCode::Down if self.select + 1 < self.filtered().len() => {
                self.select += 1;
                self.load_sel();
            }
            KeyCode::Enter | KeyCode::Right if !self.filtered().is_empty() => {
                self.focus = Focus::Form
            }
            KeyCode::Char('f') => {
                self.focus = Focus::Filter;
                self.filter.clear();
            }
            KeyCode::Char('r') => self.refresh()?,
            _ => {}
        }
        Ok(())
    }

    // ---- filter keys ----
    fn key_filter(&mut self, k: KeyEvent) -> Result<()> {
        match k.code {
            KeyCode::Esc => {
                self.focus = Focus::List;
                self.filter.clear();
            }
            KeyCode::Enter => {
                self.focus = Focus::List;
                self.load_sel();
            }
            KeyCode::Backspace => {
                self.filter.pop();
                self.select = 0;
            }
            KeyCode::Char(c) => {
                self.filter.push(c);
                self.select = 0;
            }
            _ => {}
        }
        Ok(())
    }

    // ---- form keys ----
    fn key_form(&mut self, k: KeyEvent) -> Result<()> {
        match k.code {
            KeyCode::Left => self.focus = Focus::List,
            KeyCode::Up   => self.form.cursor = (self.form.cursor + 4) % 5,
            KeyCode::Down | KeyCode::Tab => self.form.cursor = (self.form.cursor + 1) % 5,
            KeyCode::Esc  => {
                self.load_sel();
                self.focus = Focus::List;
            }
            _ => {
                if self.form.cursor == 4 {
                    // Enabled row: toggle with Space / Enter
                    if matches!(k.code, KeyCode::Char(' ') | KeyCode::Enter) {
                        self.form.enabled = !self.form.enabled;
                        self.dirty = true;
                    }
                } else {
                    let field = current_mut(&mut self.form);
                    match k.code {
                        KeyCode::Backspace => {
                            field.pop();
                            self.dirty = true;
                        }
                        KeyCode::Char(c) if c.is_ascii_graphic() => {
                            field.push(c);
                            self.dirty = true;
                        }
                        _ => {}
                    }
                }
            }
        }
        Ok(())
    }
}

/// Returns a mutable reference to the current text field in the form.
fn current_mut(f: &mut Form) -> &mut String {
    match f.cursor {
        0 => &mut f.ip,
        1 => &mut f.mask,
        2 => &mut f.gw,
        _ => &mut f.dns,
    }
}

// =================================================================================================
// Apply logic
// =================================================================================================

pub type ApplyResult = std::result::Result<String, anyhow::Error>;

pub fn do_apply(nic: &NicInfo, f: &Form) -> Result<()> {
    let ip_changed = f.ip != nic.ipv4_first.clone().unwrap_or_default()
        || f.gw != nic.gw_first.clone().unwrap_or_default()
        || f.mask != "255.255.255.0";

    if ip_changed {
        let spec = format!("{},{},{}", f.ip, f.mask, f.gw);
        platform::apply_ip(&nic.name, &spec)?;
    }

    if f.dns != nic.dns_first.clone().unwrap_or_default() {
        platform::apply_dns(&nic.name, &f.dns)?;
    }

    #[cfg(windows)]
    if f.enabled != nic.enabled {
        set_enabled(&nic.name, f.enabled)?;
    }
    Ok(())
}

impl App {
    /// Spawn background thread to apply modifications.
    pub fn start_apply(&mut self, tx: Sender<ApplyResult>, nic: NicInfo, form: Form) {
        self.busy = true;
        self.busy_since = Instant::now();
        self.spin = 0;
        self.message = None;
        thread::spawn(move || {
            let res = do_apply(&nic, &form).map(|_| nic.name.clone());
            let _ = tx.send(res);
        });
    }

    /// Handle completion of background apply.
    pub fn finish_apply(&mut self, res: ApplyResult) -> Result<()> {
        let elapsed = self.busy_since.elapsed();
        if elapsed < BUSY_MIN {
            thread::sleep(BUSY_MIN - elapsed);
        }
        self.busy = false;
        match res {
            Ok(name) => self.toast(&format!("✔ Applied to {name}")),
            Err(e)   => self.toast(&format!("❌ {e}")),
        }
        self.refresh_keep()?; // keep current selection
        self.focus = Focus::List;
        self.dirty = false;
        Ok(())
    }
}

/// Helper for UI layer: returns (field text, is_focused).
pub fn field_with_focus(f: &Form, idx: usize) -> (String, bool) {
    match idx {
        0 => (f.ip.clone(),   idx == f.cursor),
        1 => (f.mask.clone(), idx == f.cursor),
        2 => (f.gw.clone(),   idx == f.cursor),
        3 => (f.dns.clone(),  idx == f.cursor),
        _ => (
            if f.enabled { "Enabled".into() } else { "Disabled".into() },
            idx == f.cursor,
        ),
    }
}
