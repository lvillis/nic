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
    // child-only TUI mode
    if env::args().any(|a| a == "--tui") {
        let (_tx, rx) = crossbeam_channel::unbounded::<tray::Msg>();
        return console::run(rx);
    }

    // parent process
    let tui_child: Arc<Mutex<Option<Child>>> = Arc::new(Mutex::new(None));
    let child_ref = Arc::clone(&tui_child);

    tray::run_tray(move || {
        let mut guard = child_ref.lock().unwrap();

        // Windows: bring console to front if still alive
        #[cfg(windows)]
        if let Some(child) = guard.as_mut() {
            if child.try_wait().ok().flatten().is_none() {
                crate::console::show_console();
                return;
            }
        }

        // spawn new child
        if let Ok(me) = env::current_exe() {
            if let Ok(child) = Command::new(me).arg("--tui").spawn() {
                *guard = Some(child);
            }
        }
    })?;

    // ensure child killed on exit
    if let Some(mut child) = tui_child.lock().unwrap().take() {
        let _ = child.kill();
        let _ = child.wait();
    }

    Ok(())
}
