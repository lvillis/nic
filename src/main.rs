#![allow(clippy::single_match)]
//! Application bootstrap.
//!
//! * Runs in two modes  
//!   1. `--tui`  : console/TUI child used by the tray process  
//!   2. default : system-tray controller that life-cycles a TUI child
//! * Ensures the child process is terminated when tray quits.

mod app;
mod console;
mod form;
mod input;
mod platform;
mod tray;
mod ui;

use std::{
    env,
    process::{Child, Command},
    sync::{Arc, Mutex},
};

fn main() -> anyhow::Result<()> {
    // ── child mode ──────────────────────────────────────────────────────────────
    if env::args().any(|a| a == "--tui") {
        let (_tx, rx) = crossbeam_channel::unbounded::<tray::Msg>();
        return console::run(rx);
    }

    // ── tray mode ───────────────────────────────────────────────────────────────
    let tui_child: Arc<Mutex<Option<Child>>> = Arc::new(Mutex::new(None));
    let tui_child_cloned = Arc::clone(&tui_child);

    // Launch tray loop
    tray::run_tray(move || {
        let mut guard = tui_child_cloned.lock().unwrap();

        // If child is alive, just bring its window to front
        if let Some(child) = guard.as_mut() {
            if child.try_wait().ok().flatten().is_none() {
                #[cfg(windows)]
                crate::console::show_console();
                return;
            }
        }

        // Spawn fresh child
        if let Ok(me) = env::current_exe() {
            if let Ok(child) = Command::new(me).arg("--tui").spawn() {
                *guard = Some(child);
            }
        }
    })?; // returns when tray “Quit” selected

    // clean up any leftover child
    if let Some(mut child) = tui_child.lock().unwrap().take() {
        let _ = child.kill();
        let _ = child.wait();
    }

    Ok(())
}
