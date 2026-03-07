use std::io;

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

use crate::screen::Screen;
use crate::screen_state::{Direction, ScrollPlan};

/// In-memory screen for e2e testing.
/// Tracks a grid of cells and logs output on each flush.
pub struct MockScreen {
    width: u16,
    height: u16,
    /// Current screen contents, one string per row.
    grid: Vec<String>,
    /// Pre-set events to return from poll_event.
    events: Vec<Event>,
    event_index: usize,
    /// Accumulated output log.
    out: String,
}

impl MockScreen {
    pub fn new(width: u16, height: u16) -> Self {
        let grid = vec![String::new(); height as usize];
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
            self.out.push_str(row);
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
            // Log the event
            if let Event::Key(ref key) = event {
                self.log_key(key);
            }
            Ok(Some(event))
        } else {
            // Safety fallback: quit if no more events
            Ok(Some(Event::Key(KeyEvent::new(
                KeyCode::Char('q'),
                KeyModifiers::NONE,
            ))))
        }
    }

    fn draw_row(&mut self, screen_y: u16, text: &str) -> io::Result<()> {
        let y = screen_y as usize;
        if y < self.grid.len() {
            self.grid[y] = text.to_string();
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
                    self.grid.push(String::new());
                }
            }
            Direction::Up => {
                for _ in 0..n {
                    self.grid.pop();
                    self.grid.insert(0, String::new());
                }
            }
        }
        self.grid.truncate(h);
        while self.grid.len() < h {
            self.grid.push(String::new());
        }
        Ok(())
    }

    fn clear_and_flush(&mut self) -> io::Result<()> {
        let h = self.height as usize;
        self.grid = vec![String::new(); h];
        Ok(())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.snapshot();
        Ok(())
    }
}
