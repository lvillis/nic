#![cfg(windows)]

use anyhow::{Context, Result};
use crossbeam_channel::select;
use std::{sync::Arc, time::Duration};
use tray_icon::{
    menu::{Menu, MenuEvent, MenuItem},
    Icon, MouseButton, TrayIconBuilder, TrayIconEvent,
};

#[derive(Debug, Clone)]
pub enum Msg {
    Open,
    Quit,
}

const EMBED_ICON: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/logo.ico"));

fn embedded_icon() -> Result<Icon> {
    use ico::IconDir;
    let ico = IconDir::read(std::io::Cursor::new(EMBED_ICON)).context("ICO parse failed")?;
    let img = ico.entries()[0].decode().context("ICO decode failed")?;
    Ok(Icon::from_rgba(
        img.rgba_data().to_vec(),
        img.width(),
        img.height(),
    )?)
}

fn pump_win32_messages() {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        DispatchMessageW, PeekMessageW, TranslateMessage, MSG, PM_REMOVE,
    };
    unsafe {
        let mut msg: MSG = std::mem::zeroed();
        while PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

pub fn run_tray<F>(mut launch_tui: F) -> Result<()>
where
    F: FnMut() + Send + 'static,
{
    let icon = embedded_icon()?;

    let open_item = MenuItem::new("Open", true, None);
    let quit_item = MenuItem::new("Quit", true, None);

    let menu = Menu::new();
    menu.append_items(&[&open_item, &quit_item])?;

    let _tray = TrayIconBuilder::new()
        .with_icon(icon)
        .with_tooltip("nic – Network Config")
        .with_menu(Box::new(menu))
        .with_menu_on_left_click(false)
        .build()?;

    launch_tui();

    let open_id = Arc::new(open_item.id().clone());
    let quit_id = Arc::new(quit_item.id().clone());

    let tray_rx = TrayIconEvent::receiver();
    let menu_rx = MenuEvent::receiver();

    loop {
        pump_win32_messages();

        select! {
            recv(tray_rx) -> ev => {
                if let Ok(ev) = ev {
                    match ev {
                        TrayIconEvent::Click { button, .. } if button == MouseButton::Left => launch_tui(),
                        TrayIconEvent::DoubleClick { .. } => launch_tui(),
                        _ => {}
                    }
                }
            },
            recv(menu_rx) -> me => {
                if let Ok(me) = me {
                    if me.id == *open_id {
                        launch_tui();
                    } else if me.id == *quit_id {
                        break;
                    }
                }
            },
            default(Duration::from_millis(50)) => {}
        };
    }

    Ok(())
}
