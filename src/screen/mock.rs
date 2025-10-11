use std::io;

use crate::screen::ScreenSize;

use super::{Event, Key};

pub struct MockScreen {
    /// Lines written by the app but not displayed yet.
    draft: Vec<String>,
    /// Output history. The content of draft is appended on `flush()`.
    pub out: String,
    pub size: ScreenSize,
    events: Vec<Event>,
}

impl MockScreen {
    pub fn new(size: ScreenSize) -> Self {
        let mut draft = Vec::new();
        draft.resize(size.n_rows(), String::new());
        Self {
            draft,
            out: String::new(),
            size,
            events: vec![],
        }
    }

    pub fn set_events(&mut self, events: Vec<Event>) {
        self.events = events.into_iter().rev().collect()
    }
}

impl super::Screen for MockScreen {
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
        self.draft.resize(self.size.n_rows(), String::new());
        Ok(())
    }

    fn scroll_forward(&mut self, n_steps: u16) -> io::Result<()> {
        // Remove the first `n_steps` elements from the draft.
        self.draft = self.draft.split_off(n_steps as usize);
        self.draft.resize(self.size.n_rows(), String::new());
        Ok(())
    }

    fn scroll_backward(&mut self, n_steps: u16) -> io::Result<()> {
        // Prepend `n_steps` empty strings to the draft.
        let mut draft = Vec::new();
        draft.resize(self.size.n_rows(), String::new());
        let n_steps = n_steps as usize;
        for i in 0..n_steps {
            draft[i] = String::new();
        }
        for i in 0..(self.size.n_rows() - n_steps) {
            draft[i + n_steps] = self.draft[i].clone();
        }
        self.draft = draft;

        Ok(())
    }

    fn draw_at(&mut self, mut row: usize, mut line: &str) -> io::Result<()> {
        // Draw a given line with automatically wrapping it based on the column size.
        while line.len() > 0 {
            let n_cols = self.size.n_cols();
            if line.len() <= n_cols {
                self.draft[row] = line.to_string();
                break;
            }
            self.draft[row] = format!("{}>", &line[..n_cols]);
            line = &line[n_cols..];
            row += 1;
            if row >= self.size.n_rows() {
                return Err(io::Error::new(io::ErrorKind::Other, "row exceed max"));
            }
        }
        Ok(())
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
