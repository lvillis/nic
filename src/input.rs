#![allow(clippy::module_name_repetitions)]
//! Simple reusable input box widget (legacy helper).

use std::fmt;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::{Alignment, Constraint, Layout},
    text::Span,
    widgets::{Block, Borders, Paragraph},
    Frame,
};

// -------------------------------------------------------------------------------------------------
// InputBox
// -------------------------------------------------------------------------------------------------
#[derive(Default)]
pub struct InputBox {
    prompt: String,
    buf:    String,
    cursor: usize,
}

impl InputBox {
    pub fn with_prompt(prompt: &str) -> Self {
        Self { prompt: prompt.into(), ..Default::default() }
    }

    pub fn value(&self) -> String {
        self.buf.clone()
    }

    /// Returns true when Enter pressed (input confirmed).
    pub fn handle_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Char(c) => {
                self.buf.insert(self.cursor, c);
                self.cursor += 1;
            }
            KeyCode::Backspace if self.cursor > 0 => {
                self.cursor -= 1;
                self.buf.remove(self.cursor);
            }
            KeyCode::Left  if self.cursor > 0              => self.cursor -= 1,
            KeyCode::Right if self.cursor < self.buf.len() => self.cursor += 1,
            KeyCode::Enter => return true,
            _ => {}
        }
        false
    }
}

// Allow direct display inside ratatui paragraphs.
impl fmt::Display for InputBox {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.prompt, self.buf)
    }
}

// -------------------------------------------------------------------------------------------------
// Popup helpers
// -------------------------------------------------------------------------------------------------
pub fn draw_popup<T: fmt::Display>(f: &mut Frame, content: T, title: &str) {
    let area = centered_rect(60, 20, f.area());
    let block = Block::default().borders(Borders::ALL).title(title);
    let paragraph = Paragraph::new(Span::raw(content.to_string()))
        .block(block)
        .alignment(Alignment::Left);
    f.render_widget(paragraph, area);
}

fn centered_rect(px: u16, py: u16, area: ratatui::layout::Rect) -> ratatui::layout::Rect {
    let vertical = Layout::vertical([
        Constraint::Percentage((100 - py) / 2),
        Constraint::Percentage(py),
        Constraint::Percentage((100 - py) / 2),
    ])
        .split(area);
    Layout::horizontal([
        Constraint::Percentage((100 - px) / 2),
        Constraint::Percentage(px),
        Constraint::Percentage((100 - px) / 2),
    ])
        .split(vertical[1])[1]
}
