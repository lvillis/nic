#![allow(clippy::too_many_lines)]
//! TUI rendering layer (ratatui 0.30).
//!
//! * Dynamically computes left-pane width to avoid truncating status strings.  
//! * Centralizes all drawing helpers in a single module.

use crate::app::{field_with_focus, App, Focus, SPIN_FRAMES};
use crate::platform::OperStatus;
use ratatui::{
    layout::{Alignment, Constraint, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{
        Block, BorderType, Borders, Cell, Clear, Paragraph, Row, Table, TableState,
        Padding,
    },
    Frame,
};

// =================================================================
// ==============================
// main entry
// ===============================================================================================
pub fn draw(f: &mut Frame, app: &mut App) {
    // -------- compute minimal width for the NIC list --------
    let (name_max, kind_max) = calc_max_widths(app);
    // internal column spacing 1, table borders 2, plus 4 for labels
    let left_needed = (name_max + kind_max + 4 + 1 + 2) as u16;

    let total_w = f.size().width;
    let layout = if left_needed < total_w * 6 / 10 {
        // left fixed, right fills
        ratatui::layout::Layout::horizontal([
            Constraint::Length(left_needed),
            Constraint::Min(10),
        ])
            .split(f.area())
    } else {
        // fallback to 40/60 split
        ratatui::layout::Layout::horizontal([
            Constraint::Percentage(40),
            Constraint::Percentage(60),
        ])
            .split(f.area())
    };

    let left_area  = layout[0];
    let right_area = layout[1];

    draw_list(f, left_area, app, kind_max);
    draw_form(f, right_area, app);

    // -------- status bar --------
    let sb_y = f.area().bottom() - 1;
    draw_status(f, sb_y, app);

    // -------- overlays --------
    if app.confirm_save {
        draw_confirm(f, app);
    } else if app.busy {
        draw_busy(f, app);
    } else if app.show_help {
        draw_help(f);
    }

    app.expire_toast();
}

// ===============================================================================================
// left-pane list
// ===============================================================================================
fn draw_list(f: &mut Frame, area: Rect, app: &App, kind_max: usize) {
    let title = if app.focus == Focus::Filter {
        format!("NICs (filter: {}_)", app.filter)
    } else {
        "NICs (f Filter  ↑↓ Move  Enter→)".into()
    };

    let data = app.filtered();
    let rows = data.iter().map(|n| {
        let mut kind = n.kind.to_string();
        if !n.enabled {
            kind.push_str(" (disabled)");
        } else if !matches!(n.oper_status, OperStatus::IfOperStatusUp) {
            kind.push_str(" (disconnected)");
        }
        Row::new(vec![Cell::from(n.name.clone()), Cell::from(kind)])
    });

    #[allow(deprecated)]
    let table = Table::new(
        rows,
        [
            Constraint::Min(10),                 // name auto
            Constraint::Length(kind_max as u16), // status fixed
        ],
    )
        .column_spacing(1)
        .highlight_symbol("  ")
        .highlight_style(Style::default().bg(Color::Blue).fg(Color::White))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Plain)
                .title(title),
        );

    let mut state = TableState::default();
    state.select(Some(app.select));
    f.render_stateful_widget(table, area, &mut state);
}

// ===============================================================================================
// right-pane form
// ===============================================================================================
fn draw_form(f: &mut Frame, area: Rect, app: &App) {
    let labels = ["IP Address", "Subnet Mask", "Gateway", "DNS Servers", "Enabled"];

    let rows = (0..5).map(|i| {
        let (value, focused) = field_with_focus(&app.form, i);
        let mut display = value;
        if focused && app.focus == Focus::Form {
            display.push('▌');
        }

        let style = if focused && app.focus == Focus::Form {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        };

        Row::new(vec![Cell::from(labels[i]), Cell::from(display)]).style(style)
    });

    let (title, style) = if app.focus == Focus::Form {
        (
            "NIC Settings (editing)",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        ("NIC Settings", Style::default())
    };

    #[allow(deprecated)]
    let table = Table::new(rows, [Constraint::Length(14), Constraint::Min(10)])
        .column_spacing(1)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Plain)
                .style(style)
                .title(title),
        );
    f.render_widget(table, area);

    // MAC footer
    let mac = app
        .current_nic()
        .and_then(|n| n.mac.clone())
        .unwrap_or_default();

    let para = Paragraph::new(Line::from(vec![
        Span::styled("MAC: ", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(mac),
    ]))
        .alignment(Alignment::Right);
    f.render_widget(
        para,
        Rect {
            x: area.x,
            y: area.y + area.height - 1,
            width: area.width,
            height: 1,
        },
    );
}

// ===============================================================================================
// status bar
// ===============================================================================================
fn draw_status(f: &mut Frame, y: u16, app: &App) {
    let msg = if app.busy {
        format!("{}  Applying… you may keep browsing  (Esc to cancel)", SPIN_FRAMES[app.spin])
    } else if let Some((t, _)) = &app.message {
        t.clone()
    } else if app.focus == Focus::Form {
        if app.dirty {
            "Edit mode — modified  s/F10 Save  Esc Cancel".into()
        } else {
            "Edit mode — ↑↓/Tab field  Space/Enter toggle Enabled  s/F10 Save  Esc Cancel".into()
        }
    } else if app.dirty {
        "Unsaved edits — press s/F10 to save".into()
    } else {
        "↑↓ Move  Enter Edit  f Filter  r Refresh  F10/s Save  q Quit  F1 Help".into()
    };

    let bar = Paragraph::new(Span::raw(msg))
        .block(
            Block::default()
                .borders(Borders::TOP)
                .border_type(BorderType::Plain),
        )
        .alignment(Alignment::Left);
    f.render_widget(
        bar,
        Rect {
            x: 0,
            y,
            width: f.size().width,
            height: 1,
        },
    );
}

// ===============================================================================================
// help & pop-ups
// ===============================================================================================
fn draw_help(f: &mut Frame) {
    f.render_widget(Clear, f.area());
    f.render_widget(mask(), f.area());

    let area = centered_rect(70, 60, f.area());

    let help_text: Text = vec![
        Line::from(vec![
            Span::styled(
                "Keyboard Cheatsheet",
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            ),
            Span::raw("  (F1 / ? / Esc to close)"),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("List   ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw("↑↓ Move   f Filter   r Refresh   Enter/→ Form"),
        ]),
        Line::from(vec![
            Span::styled("Form   ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw("↑↓/Tab Field   Space/Enter toggle Enabled   Esc Cancel"),
        ]),
        Line::from(vec![
            Span::styled("Global ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw("s/F10 Save   q Quit   Ctrl+C Quit"),
        ]),
    ]
        .into();

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(" Help ");

    let para = Paragraph::new(help_text)
        .block(block)
        .alignment(Alignment::Left);

    f.render_widget(para, area);
}

fn draw_confirm(f: &mut Frame, app: &App) {
    f.render_widget(Clear, f.area());
    f.render_widget(mask(), f.area());

    let rows = diff_fields(app)
        .into_iter()
        .map(|(k, old, new)| Row::new(vec![Cell::from(k), Cell::from(format!("{old} → {new}"))]));

    let popup = centered_rect(60, 35, f.area());
    let layout = ratatui::layout::Layout::vertical([
        Constraint::Min(4),
        Constraint::Length(3),
    ])
        .split(popup);
    let body = layout[0];
    let hint = layout[1];

    let table = Table::new(rows, [Constraint::Length(14), Constraint::Min(20)])
        .column_spacing(1)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .padding(Padding::new(1, 1, 1, 1))
                .title("❓ Save the following changes?"),
        );
    f.render_widget(table, body);

    let tips = Paragraph::new(Span::styled(
        "Enter Save    Esc Cancel",
        Style::default().fg(Color::Black).add_modifier(Modifier::BOLD),
    ))
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .style(Style::default().bg(Color::White)),
        );
    f.render_widget(tips, hint);
}

fn draw_busy(f: &mut Frame, app: &App) {
    f.render_widget(Clear, f.area());
    f.render_widget(mask(), f.area());

    let area = centered_rect(40, 20, f.area());
    let popup = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .style(Style::default().bg(Color::White))
        .title("Saving…");

    let text = Paragraph::new(Span::styled(
        format!("{}  Applying, please wait", SPIN_FRAMES[app.spin]),
        Style::default().fg(Color::Black),
    ))
        .alignment(Alignment::Center)
        .block(popup);

    f.render_widget(text, area);
}

// ===============================================================================================
// utilities
// ===============================================================================================
fn mask() -> Block<'static> {
    Block::default().style(Style::default().bg(Color::Rgb(30, 30, 30)))
}

/// Return a centered rectangle defined by percentage of parent.
fn centered_rect(px: u16, py: u16, area: Rect) -> Rect {
    let vertical = ratatui::layout::Layout::vertical([
        Constraint::Percentage((100 - py) / 2),
        Constraint::Percentage(py),
        Constraint::Percentage((100 - py) / 2),
    ])
        .split(area);
    ratatui::layout::Layout::horizontal([
        Constraint::Percentage((100 - px) / 2),
        Constraint::Percentage(px),
        Constraint::Percentage((100 - px) / 2),
    ])
        .split(vertical[1])[1]
}

/// Return (label, old, new) for changed fields.
fn diff_fields(app: &App) -> Vec<(&'static str, String, String)> {
    let Some(nic) = app.current_nic() else { return Vec::new() };
    let mut v = Vec::new();
    if app.form.ip != nic.ipv4_first.clone().unwrap_or_default() {
        v.push(("IP Address", nic.ipv4_first.clone().unwrap_or_default(), app.form.ip.clone()));
    }
    if app.form.mask != "255.255.255.0" {
        v.push(("Subnet Mask", "255.255.255.0".into(), app.form.mask.clone()));
    }
    if app.form.gw != nic.gw_first.clone().unwrap_or_default() {
        v.push(("Gateway", nic.gw_first.clone().unwrap_or_default(), app.form.gw.clone()));
    }
    if app.form.dns != nic.dns_first.clone().unwrap_or_default() {
        v.push(("DNS Servers", nic.dns_first.clone().unwrap_or_default(), app.form.dns.clone()));
    }
    if app.form.enabled != nic.enabled {
        v.push((
            "Enabled",
            if nic.enabled { "Enabled".into() } else { "Disabled".into() },
            if app.form.enabled { "Enabled".into() } else { "Disabled".into() },
        ));
    }
    v
}

/// Compute max width of name and status columns for list table.
fn calc_max_widths(app: &App) -> (usize, usize) {
    let mut name_max = 4; // "Name"
    let mut kind_max = 5; // "Type"
    for n in &app.list {
        name_max = name_max.max(n.name.len());
        let mut kind = n.kind.to_string();
        if !n.enabled {
            kind.push_str(" (disabled)");
        } else if !matches!(n.oper_status, OperStatus::IfOperStatusUp) {
            kind.push_str(" (disconnected)");
        }
        kind_max = kind_max.max(kind.len());
    }
    (name_max, kind_max)
}
