use std::io;

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use unicode_width::UnicodeWidthChar;

use crate::ansi;
use crate::screen::Screen;
use crate::viewport::{Direction, ScrollPlan};

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

/// Recorded output entry from MockScreen.
enum OutputEntry {
    /// Key event log line (e.g. "[EVENT]:char:j\n").
    Event(String),
    /// Grid snapshot taken on flush.
    Snapshot(String),
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
    entries: Vec<OutputEntry>,
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
            entries: Vec::new(),
        }
    }

    pub fn set_events(&mut self, events: Vec<Event>) {
        self.events = events;
        self.event_index = 0;
    }

    /// Build the full output log by concatenating all entries.
    /// Each snapshot is followed by a "-----" separator.
    pub fn out(&self) -> String {
        let mut out = String::new();
        for entry in &self.entries {
            match entry {
                OutputEntry::Event(s) => out.push_str(s),
                OutputEntry::Snapshot(s) => {
                    out.push_str(s);
                    out.push_str("-----\n");
                }
            }
        }
        out
    }

    /// Returns the last grid snapshot taken on flush.
    pub fn last_snapshot(&self) -> &str {
        self.entries
            .iter()
            .rev()
            .find_map(|e| match e {
                OutputEntry::Snapshot(s) => Some(s.as_str()),
                _ => None,
            })
            .unwrap_or("")
    }

    fn log_key(&mut self, key: &KeyEvent) {
        let text = match key.code {
            KeyCode::Char(ch) => {
                if ch.is_control() {
                    format!("[EVENT]:char:{ch:?}\n")
                } else {
                    format!("[EVENT]:char:{ch}\n")
                }
            }
            KeyCode::Esc => "[EVENT]:esc\n".to_string(),
            _ => "[EVENT]:other\n".to_string(),
        };
        self.entries.push(OutputEntry::Event(text));
    }

    fn take_snapshot(&mut self) {
        let mut snap = String::new();
        for row in &self.grid {
            snap.push_str(&visualize_escapes(&row.text));
            if row.soft_wrapped {
                snap.push('>');
            }
            snap.push('\n');
        }
        self.entries.push(OutputEntry::Snapshot(snap));
    }
}

/// Replace ANSI escape sequences with readable plain text for test output.
/// This makes test failure diffs much easier to read since raw escape sequences
/// would be interpreted by the terminal.
fn visualize_escapes(text: &str) -> String {
    let mut result = String::new();
    for part in ansi::parse_text(text) {
        match part {
            ansi::Text::Control(s) => {
                result.push_str(&escape_to_label(s));
            }
            ansi::Text::Plain(s) => {
                result.push_str(s);
            }
        }
    }
    result
}

/// Convert a known escape sequence to a human-readable label.
/// Unknown sequences are shown as their debug representation.
fn escape_to_label(seq: &str) -> String {
    match seq {
        "\x1b[0m" => "{reset}".into(),
        "\x1b[1m" => "{bold}".into(),
        "\x1b[2m" => "{dim}".into(),
        "\x1b[22m" => "{/dim}".into(),
        "\x1b[3m" => "{italic}".into(),
        "\x1b[7m" => "{reverse}".into(),
        "\x1b[27m" => "{/reverse}".into(),
        "\x1b[30m" => "{black}".into(),
        "\x1b[31m" => "{red}".into(),
        "\x1b[32m" => "{green}".into(),
        "\x1b[33m" => "{yellow}".into(),
        "\x1b[34m" => "{blue}".into(),
        "\x1b[35m" => "{magenta}".into(),
        "\x1b[36m" => "{cyan}".into(),
        "\x1b[37m" => "{white}".into(),
        _ => format!("{{ESC:{}}}", seq.escape_debug()),
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

        for part in ansi::parse_text(text) {
            match part {
                ansi::Text::Control(s) => {
                    // Control sequences have zero display width; append as-is.
                    if y < self.height as usize {
                        self.grid[y].text.push_str(s);
                    }
                }
                ansi::Text::Plain(s) => {
                    for ch in s.chars() {
                        let ch_w = ch.width().unwrap_or(0);
                        if col > 0 && col + ch_w > width {
                            self.grid[y].soft_wrapped = true;
                            y += 1;
                            col = 0;
                            if y >= self.height as usize {
                                break;
                            }
                        }
                        if y >= self.height as usize {
                            break;
                        }
                        self.grid[y].text.push(ch);
                        col += ch_w;
                    }
                }
            }
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

    fn flush(&mut self) -> io::Result<()> {
        self.take_snapshot();
        Ok(())
    }
}
