use std::io;

use crate::screen::{Screen, ScreenSize};

use super::{Event, Key};

pub struct MockScreen {
    /// Lines written by the app but not displayed yet.
    draft: Vec<String>,
    /// Output history. The content of draft is appended on `flush()`.
    pub out: String,
    pub size: ScreenSize,
    events: Vec<Event>,
    cursor_pos: usize,
}

impl MockScreen {
    pub fn new(size: ScreenSize) -> Self {
        let mut draft = Vec::new();
        draft.resize(size.rows(), String::new());
        Self {
            draft,
            out: String::new(),
            size,
            events: vec![],
            cursor_pos: 0,
        }
    }

    pub fn set_events(&mut self, events: Vec<Event>) {
        self.events = events.into_iter().rev().collect()
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
                self.out.push_str(&format!("[EVENT]:char:{}\n", ch));
            }
            _ => panic!("unexpected event"),
        }
        Ok(event)
    }

    fn clear(&mut self) -> io::Result<()> {
        self.draft.clear();
        self.draft.resize(self.size.rows(), String::new());
        Ok(())
    }

    fn scroll_forward(&mut self, n_steps: u16) -> io::Result<()> {
        // Remove the first `n_steps` elements from the draft.
        self.draft = self.draft.split_off(n_steps as usize);
        self.draft.resize(self.size.rows(), String::new());
        Ok(())
    }

    fn scroll_backward(&mut self, n_steps: u16) -> io::Result<()> {
        // Prepend `n_steps` empty strings to the draft.
        let mut draft = Vec::new();
        draft.resize(self.size.rows(), String::new());
        let n_steps = n_steps as usize;
        for i in 0..n_steps {
            draft[i] = String::new();
        }
        for i in 0..(self.size.rows() - n_steps) {
            draft[i + n_steps] = self.draft[i].clone();
        }
        self.draft = draft;

        Ok(())
    }

    fn goto(&mut self, col: usize, row: usize) -> io::Result<()> {
        debug_assert!(col == 0); // Currently we don't move cursor other than col=0.
        self.cursor_pos = row;
        Ok(())
    }
}

impl io::Write for MockScreen {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let buf_len = buf.len();
        let whole_line = String::from_utf8_lossy(buf).to_string();
        let mut line = whole_line.as_str();
        let mut i_row = self.cursor_pos;

        // Draw a given line with automatically wrapping it based on the column size.
        while line.len() > 0 {
            let n_cols = self.size.cols();
            if line.len() <= n_cols {
                self.draft[i_row] = line.to_string();
                break;
            }
            self.draft[i_row] = format!("{}>", &line[..n_cols]);
            line = &line[n_cols..];
            i_row += 1;
            if i_row >= self.size.rows() {
                return Err(io::Error::new(io::ErrorKind::Other, "row exceed max"));
            }
        }

        Ok(buf_len)
    }

    fn flush(&mut self) -> io::Result<()> {
        for line in &self.draft {
            self.out.push_str(&line);
            self.out.push('\n');
        }
        self.out.push_str("-----\n");
        Ok(())
    }
}
