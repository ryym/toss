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

pub(crate) trait Screen {
    fn size(&self) -> io::Result<ScreenSize>;
    fn next_event(&mut self) -> io::Result<Event>;
    fn clear(&mut self) -> io::Result<()>;
    fn draw_at(&mut self, row: usize, line: &String) -> io::Result<()>;
    fn flush(&mut self) -> io::Result<()>;
}

#[derive(Clone, Copy)]
pub(crate) struct ScreenSize {
    n_rows: usize,
}

impl ScreenSize {
    pub(crate) fn new(n_rows: usize) -> Self {
        Self { n_rows }
    }

    pub(crate) fn n_rows(&self) -> usize {
        self.n_rows
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
        let (_n_cols, n_rows) = terminal_size()?;
        Ok(ScreenSize::new(n_rows as usize))
    }

    fn next_event(&mut self) -> io::Result<Event> {
        self.events.next().expect("failed to get input event")
    }

    fn clear(&mut self) -> io::Result<()> {
        write!(self.output, "{}{}", termion::clear::All, Goto(1, 1),)
    }

    fn draw_at(&mut self, row: usize, line: &String) -> io::Result<()> {
        write!(self.output, "{}{}", Goto(0, (row + 1) as u16), line)?;
        Ok(())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.output.flush()
    }
}
