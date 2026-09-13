use std::io::{self, Write};

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use unicode_width::UnicodeWidthChar;

use crate::ansi;
use crate::screen::{Direction, Screen, ScreenSize, Scroll};

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
///
/// The log is written to `writer` as it is produced — consumed events right
/// away, grid snapshots on flush — so a test can capture it from the same sink
/// the non-interactive output paths write to.
pub struct MockScreen<W: Write> {
    writer: W,
    size: ScreenSize,
    grid: Vec<GridRow>,
    events: Vec<Event>,
    event_index: usize,
}

impl<W: Write> MockScreen<W> {
    pub fn new(writer: W, size: ScreenSize) -> Self {
        let grid = vec![GridRow::new(); size.height()];
        Self {
            writer,
            size,
            grid,
            events: Vec::new(),
            event_index: 0,
        }
    }

    pub fn set_events(&mut self, events: Vec<Event>) {
        self.events = events;
        self.event_index = 0;
    }

    fn log_key(&mut self, key: &KeyEvent) -> io::Result<()> {
        let mut modifiers = key.modifiers;
        let kind = match key.code {
            KeyCode::Char(ch) => {
                // Shift is already reflected in the character itself.
                modifiers = modifiers.difference(KeyModifiers::SHIFT);
                if ch.is_control() {
                    format!("char:{ch:?}")
                } else {
                    format!("char:{ch}")
                }
            }
            KeyCode::Esc => "esc".to_string(),
            KeyCode::Enter => "enter".to_string(),
            KeyCode::Backspace => "backspace".to_string(),
            KeyCode::Left => "left".to_string(),
            KeyCode::Right => "right".to_string(),
            _ => {
                let text = format!("[EVENT]:ERROR:unexpected:{}\n", key.code);
                return self.writer.write_all(text.as_bytes());
            }
        };
        let text = format!("[EVENT]:{}{kind}\n", modifiers_prefix(modifiers));
        self.writer.write_all(text.as_bytes())
    }

    /// Simulate a terminal resize: update the tracked size and the grid to match,
    /// then log the event so it shows up in the output like a key event does.
    fn log_resize(&mut self, width: u16, height: u16) -> io::Result<()> {
        self.size = ScreenSize::new(width, height);
        self.grid.resize(self.size.height(), GridRow::new());
        writeln!(self.writer, "[EVENT]:resize:{width}x{height}")
    }

    /// Write the current grid as a snapshot, terminated by a "-----" separator.
    fn take_snapshot(&mut self) -> io::Result<()> {
        let mut snap = String::new();
        for row in &self.grid {
            snap.push_str(&visualize_escapes(&row.text));
            if row.soft_wrapped {
                snap.push('>');
            }
            snap.push('\n');
        }
        snap.push_str("-----\n");
        self.writer.write_all(snap.as_bytes())
    }
}

/// Build the prefix that precedes a key kind in the event log, e.g. `ctrl+alt+`, or an empty
/// string when there are no modifiers.
fn modifiers_prefix(modifiers: KeyModifiers) -> String {
    let names = [
        (KeyModifiers::CONTROL, "ctrl"),
        (KeyModifiers::ALT, "alt"),
        (KeyModifiers::SHIFT, "shift"),
        (KeyModifiers::SUPER, "super"),
        (KeyModifiers::HYPER, "hyper"),
        (KeyModifiers::META, "meta"),
    ];
    names
        .iter()
        .filter(|(modifier, _)| modifiers.contains(*modifier))
        .map(|(_, name)| format!("{name}+"))
        .collect()
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
        "\x1b[1m" => "{b}".into(), // bold
        "\x1b[22m" => "{/b}".into(),
        "\x1b[4m" => "{line}".into(), // underline
        "\x1b[24m" => "{/line}".into(),
        "\x1b[7m" => "{rev}".into(), // reverse
        "\x1b[27m" => "{/rev}".into(),
        "\x1b[31m" => "{red}".into(),
        _ => format!("{{ESC:{}}}", seq.escape_debug()),
    }
}

impl<W: Write> Screen for MockScreen<W> {
    fn size(&self) -> io::Result<ScreenSize> {
        Ok(self.size)
    }

    fn poll_event(&mut self, _timeout: std::time::Duration) -> io::Result<Option<Event>> {
        if self.event_index < self.events.len() {
            let event = self.events[self.event_index].clone();
            self.event_index += 1;
            match &event {
                Event::Key(key) => self.log_key(key)?,
                Event::Resize(w, h) => self.log_resize(*w, *h)?,
                _ => {}
            }
            Ok(Some(event))
        } else {
            panic!("MockScreen ran out of scripted events without the app quitting");
        }
    }

    fn clear_row(&mut self, screen_y: usize) -> io::Result<()> {
        if screen_y < self.grid.len() {
            self.grid[screen_y] = GridRow::new();
        }
        Ok(())
    }

    fn write_at(&mut self, screen_y: usize, text: &str) -> io::Result<()> {
        let mut y = screen_y;
        let mut col = 0;

        for part in ansi::parse_text(text) {
            match part {
                ansi::Text::Control(s) => {
                    // Control sequences have zero display width; append as-is.
                    if y < self.size.height() {
                        self.grid[y].text.push_str(s);
                    }
                }
                ansi::Text::Plain(s) => {
                    for ch in s.chars() {
                        let ch_w = ch.width().unwrap_or(0);
                        if col > 0 && col + ch_w > self.size.width() {
                            self.grid[y].soft_wrapped = true;
                            y += 1;
                            col = 0;
                            if y >= self.size.height() {
                                break;
                            }
                        }
                        if y >= self.size.height() {
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

    fn scroll_terminal(&mut self, scroll: &Scroll) -> io::Result<()> {
        let num_rows = scroll.num_rows.get();
        match scroll.direction {
            Direction::Down => {
                // Content moves up: remove n rows from top, add blank at bottom.
                let remove = num_rows.min(self.size.height());
                for _ in 0..remove {
                    self.grid.remove(0);
                    self.grid.push(GridRow::new());
                }
            }
            Direction::Up => {
                // Content moves down: remove n rows from bottom, add blank at top.
                let remove = num_rows.min(self.size.height());
                for _ in 0..remove {
                    self.grid.pop();
                    self.grid.insert(0, GridRow::new());
                }
            }
        }
        Ok(())
    }

    fn begin_sync(&mut self) -> io::Result<()> {
        Ok(())
    }

    fn end_sync(&mut self) -> io::Result<()> {
        Ok(())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.take_snapshot()
    }
}
