#[cfg(test)]
pub mod mock;

use std::fs::File;
use std::io;

use termion::cursor::Goto;
use termion::input::{Events, TermRead};
use termion::raw::IntoRawMode;
use termion::screen::IntoAlternateScreen;
use termion::terminal_size;

pub(crate) use termion::event::{Event, Key};

pub(crate) trait Screen: io::Write {
    fn size(&self) -> io::Result<ScreenSize>;
    fn next_event(&mut self) -> io::Result<Event>;
    fn clear(&mut self) -> io::Result<()>;
    fn scroll_forward(&mut self, n_steps: u16) -> io::Result<()>;
    fn scroll_backward(&mut self, n_steps: u16) -> io::Result<()>;
    /// Move the cursor position. Indice are 0-based.
    fn goto(&mut self, col: usize, row: usize) -> io::Result<()>;
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ScreenSize {
    /// A number of columns of the screen.
    cols: usize,
    /// A number of rows of the screen.
    rows: usize,
}

impl ScreenSize {
    pub(crate) fn new(cols: usize, rows: usize) -> Self {
        Self { cols, rows }
    }

    pub(crate) fn cols(&self) -> usize {
        self.cols
    }

    pub(crate) fn rows(&self) -> usize {
        self.rows
    }
}

pub(crate) struct TerminalScreen<R: io::Read, W: io::Write> {
    events: Events<R>,
    output: W,
}

pub(crate) fn for_terminal() -> io::Result<TerminalScreen<impl io::Read, impl io::Write>> {
    let stdout = io::stdout().lock();
    let stdout = stdout.into_raw_mode()?;
    let input_tty = File::open("/dev/tty")?;
    let events = input_tty.events();
    let alt_screen = stdout.into_alternate_screen()?;
    let no_cursor = termion::cursor::HideCursor::from(alt_screen);
    TerminalScreen::new(events, no_cursor)
}

impl<R: io::Read, W: io::Write> TerminalScreen<R, W> {
    fn new(events: Events<R>, output: W) -> io::Result<Self> {
        Ok(Self { events, output })
    }
}

impl<R: io::Read, W: io::Write> Screen for TerminalScreen<R, W> {
    fn size(&self) -> io::Result<ScreenSize> {
        let (n_cols, n_rows) = terminal_size()?;
        Ok(ScreenSize::new(n_cols as usize, n_rows as usize))
    }

    fn next_event(&mut self) -> io::Result<Event> {
        self.events.next().expect("failed to get input event")
    }

    fn clear(&mut self) -> io::Result<()> {
        write!(self.output, "{}{}", termion::clear::All, Goto(1, 1),)
    }

    fn scroll_forward(&mut self, n_steps: u16) -> io::Result<()> {
        write!(self.output, "{}", termion::scroll::Up(n_steps))
    }

    fn scroll_backward(&mut self, n_steps: u16) -> io::Result<()> {
        write!(self.output, "{}", termion::scroll::Down(n_steps))
    }

    fn goto(&mut self, col: usize, row: usize) -> io::Result<()> {
        write!(self.output, "{}", Goto((col + 1) as u16, (row + 1) as u16))
    }
}

impl<R: io::Read, W: io::Write> io::Write for TerminalScreen<R, W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.output.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.output.flush()
    }
}
