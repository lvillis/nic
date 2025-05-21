#![allow(clippy::too_many_arguments)]
//! TUI entry point and console window management.
//!
//! On Windows the console is hidden to system tray; this file also handles that logic.

use anyhow::Result;
use crossbeam_channel::{after, select, unbounded, Receiver, Sender};
use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyEventKind,
        KeyModifiers,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::{io, panic, thread, time::Duration};

use crate::{
    app::{App, SPIN_FRAMES},
    tray::Msg as TrayMsg,
    ui,
};

// -------------------------------------------------------------------------------------------------
// Windows-specific helpers: show/hide/title
// -------------------------------------------------------------------------------------------------
#[cfg(windows)]
fn console_hwnd() -> Option<windows_sys::Win32::Foundation::HWND> {
    use windows_sys::Win32::System::Console::GetConsoleWindow;
    let hwnd = unsafe { GetConsoleWindow() };
    if hwnd != std::ptr::null_mut() { Some(hwnd) } else { None }
}

#[cfg(windows)]
fn hide_console() {
    use windows_sys::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_HIDE};
    if let Some(h) = console_hwnd() {
        // Full hide: task-bar button disappears as well
        unsafe { ShowWindow(h, SW_HIDE) };
    }
}

/// Public for parent process: restore a previously hidden console and bring to front.
#[cfg(windows)]
pub(crate) fn show_console() {
    use windows_sys::Win32::UI::WindowsAndMessaging::{SetForegroundWindow, ShowWindow, SW_RESTORE};
    if let Some(h) = console_hwnd() {
        unsafe {
            ShowWindow(h, SW_RESTORE);
            SetForegroundWindow(h);
        }
    }
    set_console_title("nic – Network Config");
}

#[cfg(not(windows))]
fn hide_console() {}
#[cfg(not(windows))]
pub(crate) fn show_console() {}

/// Set console title (Windows only).
#[cfg(windows)]
fn set_console_title(title: &str) {
    use std::{ffi::OsStr, os::windows::prelude::OsStrExt};
    use windows_sys::Win32::System::Console::SetConsoleTitleW;
    let wide: Vec<u16> = OsStr::new(title).encode_wide().chain(Some(0)).collect();
    unsafe { SetConsoleTitleW(wide.as_ptr()) };
}
#[cfg(not(windows))]
fn set_console_title(_: &str) {}

// -------------------------------------------------------------------------------------------------
// Unified event enum passed around the main loop
// -------------------------------------------------------------------------------------------------
enum Ev {
    Key(KeyEvent),
    Tick,
    Apply(crate::app::ApplyResult),
    Tray(TrayMsg),
}

// -------------------------------------------------------------------------------------------------
// Spawn blocking keyboard input thread
// -------------------------------------------------------------------------------------------------
fn spawn_input(tx: Sender<Ev>) {
    thread::spawn(move || loop {
        if event::poll(Duration::from_millis(100)).unwrap() {
            if let Event::Key(k) = event::read().unwrap() {
                if k.kind == KeyEventKind::Press {
                    let _ = tx.send(Ev::Key(k));
                }
            }
        }
    });
}

// -------------------------------------------------------------------------------------------------
// Helper trait to pipe ApplyResult into Ev::Apply on the main channel
// -------------------------------------------------------------------------------------------------
trait SinkApply {
    fn sink_apply(self) -> Sender<crate::app::ApplyResult>;
}
impl SinkApply for Sender<Ev> {
    fn sink_apply(self) -> Sender<crate::app::ApplyResult> {
        let (tx, rx) = unbounded::<crate::app::ApplyResult>();
        thread::spawn(move || for r in rx {
            let _ = self.send(Ev::Apply(r));
        });
        tx
    }
}

// -------------------------------------------------------------------------------------------------
fn cleanup_terminal() {
    let _ = disable_raw_mode();
    let _ = execute!(
        io::stdout(),
        LeaveAlternateScreen,
        DisableMouseCapture,
        crossterm::terminal::Clear(crossterm::terminal::ClearType::All),
        crossterm::style::ResetColor
    );
}

/// Drop guard that restores terminal regardless of how we exit.
struct TermGuard;
impl Drop for TermGuard {
    fn drop(&mut self) {
        cleanup_terminal();
    }
}

// =================================================================================================
// Main entry used by tray module
// =================================================================================================
pub fn run(tray_rx: Receiver<TrayMsg>) -> Result<()> {
    // Ensure window is visible and title is set before TUI start
    #[cfg(windows)]
    {
        show_console();
    }
    #[cfg(not(windows))]
    {
        set_console_title("nic – Network Config");
    }

    // Panic hook: restore terminal then print panic info.
    panic::set_hook(Box::new(|info| {
        cleanup_terminal();
        eprintln!("{info}");
    }));

    // Ctrl-C handler to cleanup and exit.
    ctrlc::set_handler(|| {
        cleanup_terminal();
        std::process::exit(0);
    })
        .expect("set Ctrl-C handler");

    let _guard = TermGuard;

    // ----- boot TUI -----
    enable_raw_mode()?;
    execute!(io::stdout(), EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(io::stdout());
    let mut term = Terminal::new(backend)?;

    let (ev_tx, ev_rx) = unbounded::<Ev>();
    spawn_input(ev_tx.clone());

    // Forward tray events
    let tray_forwarder = ev_tx.clone();
    thread::spawn(move || {
        for m in tray_rx {
            let _ = tray_forwarder.send(Ev::Tray(m));
        }
    });

    let mut app = App::init()?;
    const TICK: Duration = Duration::from_millis(120);

    // Track whether console is visible; we stop drawing when hidden.
    let mut visible = true;

    loop {
        // 1) draw frame
        if visible {
            term.draw(|f| ui::draw(f, &mut app))?;
        }

        // 2) wait for event or tick
        select! {
            recv(ev_rx) -> msg => {
                if let Ok(ev) = msg {
                    // handle_event returns true when TUI must exit
                    if handle_event(ev, &mut app, &ev_tx, &mut visible, &mut term)? {
                        break;
                    }
                }
            },
            recv(after(TICK)) -> _ => {
                if app.busy {
                    app.spin = (app.spin + 1) % SPIN_FRAMES.len();
                }
            }
        }
    }

    Ok(())
}

// -------------------------------------------------------------------------------------------------
// Event dispatcher
// -------------------------------------------------------------------------------------------------
fn handle_event(
    ev: Ev,
    app: &mut App,
    ev_tx: &Sender<Ev>,
    visible: &mut bool,
    _term: &mut Terminal<CrosstermBackend<io::Stdout>>,
) -> Result<bool> {
    match ev {
        // ----- keyboard -----
        Ev::Key(k) => {
            // Ctrl+C: global exit when allowed
            if k.code == KeyCode::Char('c')
                && k.modifiers.contains(KeyModifiers::CONTROL)
                && app.allow_quit()
            {
                cleanup_terminal();
                std::process::exit(0);
            }

            // Esc cancels spinner
            if app.busy && k.code == KeyCode::Esc {
                app.busy = false;
                return Ok(false);
            }

            // Confirmation popup
            if app.confirm_save {
                match k.code {
                    KeyCode::Enter => {
                        if let Some(nic) = app.current_nic().cloned() {
                            let form = app.form.clone();
                            let apply_tx = ev_tx.clone().sink_apply();
                            app.start_apply(apply_tx, nic, form);
                        }
                        app.confirm_save = false;
                    }
                    KeyCode::Esc => app.confirm_save = false,
                    _ => {}
                }
                return Ok(false);
            }

            // Global shortcuts from list/form/filter
            match k.code {
                // save
                KeyCode::Char('s') | KeyCode::F(10) if !app.busy => {
                    if app.dirty {
                        app.confirm_save = true;
                    }
                    return Ok(false);
                }
                // quit to tray -> hide console and exit loop
                KeyCode::Char('q') if app.allow_quit() => {
                    cleanup_terminal();
                    hide_console();
                    *visible = false;
                    return Ok(true);
                }
                _ => {}
            }

            // Delegate to state machine
            app.on_key(k)?;
        }

        // ----- background apply finished -----
        Ev::Apply(r) => {
            app.finish_apply(r)?;
        }

        // ----- tray messages -----
        Ev::Tray(m) => match m {
            TrayMsg::Open => {
                // Should never hit: tray relaunches a new process instead
                let _ = m;
            }
            TrayMsg::Quit => {
                cleanup_terminal();
                std::process::exit(0);
            }
        },

        // tick ignored (handled in main loop)
        Ev::Tick => {}
    }
    Ok(false)
}
