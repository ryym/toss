use std::io;

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use unicode_width::UnicodeWidthChar;

use crate::screen::Screen;
use crate::screen_state::{Direction, ScrollPlan};

#[derive(Debug, Clone)]
struct GridRow {
    text: String,
    /// True if this row overflowed and wraps to the next row (soft wrap).
    soft_wrapped: bool,
}

impl GridRow {
    fn new() -> Self {
        Self {
            text: String::new(),
            soft_wrapped: false,
        }
    }
}

/// In-memory screen for e2e testing.
/// Tracks a grid of cells and logs output on each flush.
/// Simulates soft wrapping: when write_at overflows a row, it continues
/// to the next row and marks the overflow row with '>'.
pub struct MockScreen {
    width: u16,
    height: u16,
    grid: Vec<GridRow>,
    events: Vec<Event>,
    event_index: usize,
    out: String,
}

impl MockScreen {
    pub fn new(width: u16, height: u16) -> Self {
        let grid = vec![GridRow::new(); height as usize];
        Self {
            width,
            height,
            grid,
            events: Vec::new(),
            event_index: 0,
            out: String::new(),
        }
    }

    pub fn set_events(&mut self, events: Vec<Event>) {
        self.events = events;
        self.event_index = 0;
    }

    pub fn out(&self) -> &str {
        &self.out
    }

    fn log_key(&mut self, key: &KeyEvent) {
        match key.code {
            KeyCode::Char(ch) => {
                if ch.is_control() {
                    self.out.push_str(&format!("[EVENT]:char:{ch:?}\n"));
                } else {
                    self.out.push_str(&format!("[EVENT]:char:{ch}\n"));
                }
            }
            KeyCode::Esc => self.out.push_str("[EVENT]:esc\n"),
            _ => self.out.push_str("[EVENT]:other\n"),
        }
    }

    fn snapshot(&mut self) {
        for row in &self.grid {
            self.out.push_str(&row.text);
            if row.soft_wrapped {
                self.out.push('>');
            }
            self.out.push('\n');
        }
        self.out.push_str("-----\n");
    }
}

impl Screen for MockScreen {
    fn size(&self) -> io::Result<(u16, u16)> {
        Ok((self.width, self.height))
    }

    fn poll_event(&mut self, _timeout: std::time::Duration) -> io::Result<Option<Event>> {
        if self.event_index < self.events.len() {
            let event = self.events[self.event_index].clone();
            self.event_index += 1;
            if let Event::Key(ref key) = event {
                self.log_key(key);
            }
            Ok(Some(event))
        } else {
            Ok(Some(Event::Key(KeyEvent::new(
                KeyCode::Char('q'),
                KeyModifiers::NONE,
            ))))
        }
    }

    fn clear_row(&mut self, screen_y: u16) -> io::Result<()> {
        let y = screen_y as usize;
        if y < self.grid.len() {
            self.grid[y] = GridRow::new();
        }
        Ok(())
    }

    fn write_at(&mut self, screen_y: u16, text: &str) -> io::Result<()> {
        let width = self.width as usize;
        let mut y = screen_y as usize;
        let mut col = 0;

        for ch in text.chars() {
            let ch_w = ch.width().unwrap_or(0);
            if col > 0 && col + ch_w > width {
                // Overflow: mark current row as soft-wrapped, move to next
                self.grid[y].soft_wrapped = true;
                y += 1;
                col = 0;
                if y >= self.height as usize {
                    break;
                }
            }
            self.grid[y].text.push(ch);
            col += ch_w;
        }
        Ok(())
    }

    fn scroll_terminal(&mut self, plan: &ScrollPlan) -> io::Result<()> {
        let n = plan.terminal_scroll;
        let h = self.height as usize;
        match plan.direction {
            Direction::Down => {
                for _ in 0..n {
                    self.grid.remove(0);
                    self.grid.push(GridRow::new());
                }
            }
            Direction::Up => {
                for _ in 0..n {
                    self.grid.pop();
                    self.grid.insert(0, GridRow::new());
                }
            }
        }
        self.grid.truncate(h);
        while self.grid.len() < h {
            self.grid.push(GridRow::new());
        }
        Ok(())
    }

    fn clear_all(&mut self) -> io::Result<()> {
        let h = self.height as usize;
        self.grid = vec![GridRow::new(); h];
        Ok(())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.snapshot();
        Ok(())
    }
}
