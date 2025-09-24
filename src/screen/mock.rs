use std::io;

use crate::screen::ScreenSize;

use super::{Event, Key};

pub struct MockScreen {
    pub out: String,
    pub size: ScreenSize,
    events: Vec<Event>,
}

impl MockScreen {
    pub fn new(size: ScreenSize) -> Self {
        Self {
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
        self.out.push_str("[CLEAR]\n");
        Ok(())
    }

    fn draw(&mut self, lines: &[String]) -> io::Result<()> {
        for line in lines {
            self.out.push_str(&line);
            self.out.push('\n');
        }
        Ok(())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.out.push_str("-----\n");
        Ok(())
    }
}
