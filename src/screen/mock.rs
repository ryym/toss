//! This module provides [`MockScreen`] for testing the application without actual terminal.
//! This mock is complex enough to require its own tests, but we think it is simpler and
//! easier to debug than building automated tests using actual terminal.

use std::io;

use termion::event::{Event, Key};
use unicode_width::UnicodeWidthChar;

use crate::{
    screen::{Screen, ScreenSize},
    terminal,
};

#[derive(Debug)]
struct CursorPos {
    /// Index of [`MockScreen::draft`].
    row: usize,
    /// Index of [`MockRow::cells`].
    col: usize,
}

/// A cell constitutes a [`MockRow`]. See [`MockScreen`] for details.
#[derive(Debug, Clone)]
struct CellChar {
    value: char,
    width: usize,
    seq_index: usize,
}

impl CellChar {
    fn new() -> Self {
        Self {
            value: ' ',
            width: 1,
            seq_index: 0,
        }
    }
}

#[derive(Debug, Clone)]
struct Cell {
    control: String,
    char: CellChar,
}

impl Cell {
    fn new() -> Self {
        Self {
            control: String::new(),
            char: CellChar::new(),
        }
    }
}

/// A screen row which consists of one or more [`Cell`]s. See [`MockScreen`] for details.
#[derive(Debug, Clone)]
struct MockRow {
    cells: Vec<Cell>,
    end_control: String,
    is_wrapped: bool,
}

impl MockRow {
    fn new(col_size: usize) -> Self {
        let mut cells = Vec::new();
        cells.resize(col_size, Cell::new());
        Self {
            cells,
            end_control: String::new(),
            is_wrapped: false,
        }
    }

    fn write_control(&mut self, cell_idx: usize, control: &str) {
        if cell_idx == self.cells.len() {
            self.end_control = control.to_string();
        } else if cell_idx < self.cells.len() {
            self.cells[cell_idx].control = control.to_string();
        } else {
            panic!("cell exceed max: {cell_idx}");
        }
    }

    /// Try to write a character. It successfully writes only if the row has enough capacity.
    /// If it doesn't, it returns `None`.
    fn write_char(&mut self, cell_idx: usize, c: char) -> Option<usize> {
        let width = c.width().unwrap_or(0);
        let end_cell_idx = cell_idx + width - 1;
        if self.cells.len() <= end_cell_idx {
            return None;
        }
        for i in cell_idx..=end_cell_idx {
            self.clear_char_at(i);
        }
        for i in 0..width {
            self.cells[cell_idx + i].char = CellChar {
                value: c,
                width: width,
                seq_index: i,
            };
        }
        Some(width)
    }

    fn clear_char_at(&mut self, cell_idx: usize) {
        let cell = &self.cells[cell_idx];
        let start_idx = cell_idx - cell.char.seq_index;
        let end = start_idx + cell.char.width;
        for i in start_idx..end {
            self.cells[i].char = CellChar::new();
        }
    }

    fn clear_after(&mut self, cell_idx: usize) {
        for i in cell_idx..self.cells.len() {
            self.cells[i] = Cell::new();
        }
    }

    fn mark_wrap(&mut self) {
        self.is_wrapped = true;
    }

    fn to_string(&self) -> String {
        let mut s = String::new();
        for cell in &self.cells {
            if cell.char.seq_index == 0 {
                s.push_str(&visualize_control(&cell.control));
                s.push(cell.char.value);
            }
        }
        s = s.trim_end().to_string();
        s.push_str(&visualize_control(&self.end_control));
        if self.is_wrapped {
            s.push_str(">");
        }
        s
    }
}

fn visualize_control(s: &str) -> String {
    // Replace all possible control sequences to a plain string for easy testing.
    // In real world, there are infinite patterns of control sequences.
    // But patterns used in tests are limited.
    s.replace("\x1b[7m", "[[").replace("\x1b[27m", "]]")
}

/// An in-memory implementation of [`Screen`] purely for testing purpose.
/// [`MockScreen`] consists of [`MockRow`]s and a row consists of [`Cell`]s.
/// A visible character occupies one or more cells while control sequences don't.
#[derive(Debug)]
pub(crate) struct MockScreen {
    draft: Vec<MockRow>,
    size: ScreenSize,
    events: Vec<Event>,
    cursor: CursorPos,
    out: String,
}

fn init_draft(draft: &mut Vec<MockRow>, size: &ScreenSize) {
    draft.resize(size.rows(), MockRow::new(size.cols()));
}

impl MockScreen {
    pub fn new(size: ScreenSize) -> Self {
        let mut draft = Vec::new();
        init_draft(&mut draft, &size);
        Self {
            draft,
            size,
            events: Vec::new(),
            cursor: CursorPos { row: 0, col: 0 },
            out: String::new(),
        }
    }

    /// Prints all output histories as a String including emitted events and control sequences.
    #[inline]
    pub fn out(&self) -> &String {
        &self.out
    }

    /// Pre-set events. Events will be fired in order on [`Screen::next_event`].
    pub fn set_events(&mut self, events: Vec<Event>) {
        self.events = events.into_iter().rev().collect()
    }
}

impl io::Write for MockScreen {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let text = str::from_utf8(buf).unwrap();
        let pos = &mut self.cursor;
        let mut row = &mut self.draft[pos.row];
        for part in terminal::parse_text(text) {
            match part {
                terminal::Text::Control(s) => {
                    row.write_control(pos.col, s);
                }
                terminal::Text::Plain(s) => {
                    for c in s.chars() {
                        loop {
                            match row.write_char(pos.col, c) {
                                Some(width) => {
                                    pos.col += width;
                                    break;
                                }
                                None => {
                                    row.clear_after(pos.col);
                                    row.mark_wrap();
                                    if pos.row == self.size.rows() - 1 {
                                        panic!("row exceed max: {}", pos.row);
                                    }
                                    pos.col = 0;
                                    pos.row += 1;
                                    row = &mut self.draft[pos.row];
                                }
                            }
                        }
                    }
                }
            }
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        for row in &self.draft {
            self.out.push_str(&row.to_string());
            self.out.push('\n');
        }
        self.out.push_str("-----\n");
        Ok(())
    }
}

impl Screen for MockScreen {
    fn size(&self) -> io::Result<ScreenSize> {
        Ok(self.size)
    }

    fn next_event(&mut self) -> io::Result<Event> {
        let event = self.events.pop().unwrap();
        match event {
            Event::Key(Key::Char(ch)) => {
                let event_log = if ch.is_control() {
                    format!("[EVENT]:char:{:?}\n", ch)
                } else {
                    format!("[EVENT]:char:{}\n", ch)
                };
                self.out.push_str(&event_log);
            }
            _ => panic!("unexpected event"),
        }
        Ok(event)
    }

    fn clear(&mut self) -> io::Result<()> {
        self.draft.clear();
        init_draft(&mut self.draft, &self.size);
        Ok(())
    }

    fn scroll_forward(&mut self, n_steps: u16) -> io::Result<()> {
        // Remove the first `n_steps` elements from the draft.
        self.draft = self.draft.split_off(n_steps as usize);
        init_draft(&mut self.draft, &self.size);
        Ok(())
    }

    fn scroll_backward(&mut self, n_steps: u16) -> io::Result<()> {
        // Prepend `n_steps` empty strings to the draft.
        let mut draft = Vec::new();
        init_draft(&mut draft, &self.size);
        let n_steps = n_steps as usize;
        for i in 0..(self.size.rows() - n_steps) {
            draft[i + n_steps] = self.draft[i].clone();
        }
        self.draft = draft;
        Ok(())
    }

    fn goto(&mut self, col: usize, row: usize) -> io::Result<()> {
        self.cursor = CursorPos { row, col };
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use indoc::indoc;
    use pretty_assertions::assert_eq;
    use termion::event::{Event, Key};

    use crate::{
        error::AnyError,
        screen::{Screen, ScreenSize, mock::MockScreen},
    };

    #[test]
    fn write_lines() -> Result<(), AnyError> {
        let mut screen = MockScreen::new(ScreenSize::new(5, 3));
        screen.write(b"abc")?;
        screen.goto(0, 1)?;
        screen.write(b"de")?;
        screen.write(b"fgh")?;

        assert_eq!(screen.out(), "");
        screen.flush()?;

        let want = indoc! {"
            abc
            defgh

            -----
        "};
        assert_eq!(screen.out(), want);
        Ok(())
    }

    #[test]
    fn write_and_wrap_lines() -> Result<(), AnyError> {
        let mut screen = MockScreen::new(ScreenSize::new(5, 3));
        screen.write(b"abcdefg")?;
        screen.flush()?;
        screen.goto(0, 0)?;
        screen.write("あいうえ".as_bytes())?;
        screen.flush()?;

        let want = indoc! {"
            abcde>
            fg

            -----
            あい>
            うえ

            -----
        "};
        assert_eq!(screen.out(), want);
        Ok(())
    }

    #[test]
    fn write_line_with_control() -> Result<(), AnyError> {
        let mut screen = MockScreen::new(ScreenSize::new(3, 3));
        screen.write("\x1b[7mabc\x1b[27m".as_bytes())?;
        screen.flush()?;
        screen.goto(0, 1)?;
        screen.write("12\x1b[7m34\x1b[27m5".as_bytes())?;
        screen.flush()?;

        let want = indoc! {"
            [[abc]]


            -----
            [[abc]]
            12[[3>
            4]]5
            -----
        "};
        assert_eq!(screen.out(), want);
        Ok(())
    }

    #[test]
    fn overwrite_lines() -> Result<(), AnyError> {
        let mut screen = MockScreen::new(ScreenSize::new(10, 1));

        screen.write(b"abcdefg")?;
        screen.goto(0, 0)?;
        screen.write(b"AB")?;
        screen.flush()?;

        screen.goto(0, 0)?;
        screen.write("あいう".as_bytes())?;
        screen.flush()?;

        screen.goto(2, 0)?;
        screen.write("i".as_bytes())?;
        screen.goto(5, 0)?;
        screen.write("u".as_bytes())?;
        screen.flush()?;

        let want = indoc! {"
            ABcdefg
            -----
            あいうg
            -----
            あi  ug
            -----
        "};
        assert_eq!(screen.out(), want);

        Ok(())
    }

    #[test]
    fn record_events() -> Result<(), AnyError> {
        let mut screen = MockScreen::new(ScreenSize::new(5, 3));
        screen.set_events(vec![Event::Key(Key::Char('a')), Event::Key(Key::Char('q'))]);
        screen.write(b"1234")?;
        screen.flush()?;
        screen.next_event()?;
        screen.goto(0, 1)?;
        screen.write(b"5678")?;
        screen.flush()?;
        screen.next_event()?;

        assert_eq!(
            screen.out(),
            indoc! {"
            1234


            -----
            [EVENT]:char:a
            1234
            5678

            -----
            [EVENT]:char:q
        "}
        );
        Ok(())
    }
}
