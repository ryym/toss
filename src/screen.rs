use std::io;

use crossterm::event::Event;

mod term_screen;

pub use term_screen::TermScreen;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScreenSize {
    width: usize,
    height: usize,
}

impl ScreenSize {
    pub fn new(width: u16, height: u16) -> Self {
        Self {
            width: width as usize,
            height: height as usize,
        }
    }

    #[inline]
    pub fn width(&self) -> usize {
        self.width
    }

    #[inline]
    pub fn height(&self) -> usize {
        self.height
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Down,
    Up,
}

#[derive(Debug, Clone, Copy)]
pub struct Scroll {
    pub direction: Direction,
    pub num_rows: usize,
}

/// Abstract terminal operations for rendering and input.
pub trait Screen {
    fn size(&self) -> io::Result<ScreenSize>;
    fn poll_event(&mut self, timeout: std::time::Duration) -> io::Result<Option<Event>>;

    /// Clear a single row.
    fn clear_row(&mut self, screen_y: u16) -> io::Result<()>;

    /// Write text starting at (0, screen_y). If the text overflows the terminal
    /// width, the terminal wraps it to subsequent rows as soft wraps.
    /// Caller must clear target rows beforehand.
    fn write_at(&mut self, screen_y: u16, text: &str) -> io::Result<()>;

    /// Issue a terminal scroll command to shift content in-place.
    fn scroll_terminal(&mut self, scroll: &Scroll) -> io::Result<()>;

    /// Begin synchronized output. The terminal buffers all subsequent writes
    /// until `end_sync` and renders them in a single frame.
    fn begin_sync(&mut self) -> io::Result<()>;

    /// End synchronized output and let the terminal render the buffered frame.
    fn end_sync(&mut self) -> io::Result<()>;

    fn flush(&mut self) -> io::Result<()>;
}
