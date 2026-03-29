use std::io::{self, Stdout, Write};

use crossterm::{
    cursor,
    event::{self, Event},
    execute, queue,
    style::Print,
    terminal::{self, BeginSynchronizedUpdate, ClearType, EndSynchronizedUpdate},
};

use crate::page::{Direction, ScrollPlan};

/// Abstract terminal operations for rendering and input.
pub trait Screen {
    fn size(&self) -> io::Result<(u16, u16)>;
    fn poll_event(&mut self, timeout: std::time::Duration) -> io::Result<Option<Event>>;

    /// Clear a single row.
    fn clear_row(&mut self, screen_y: u16) -> io::Result<()>;

    /// Write text starting at (0, screen_y). If the text overflows the terminal
    /// width, the terminal wraps it to subsequent rows as soft wraps.
    /// Caller must clear target rows beforehand.
    fn write_at(&mut self, screen_y: u16, text: &str) -> io::Result<()>;

    /// Issue a terminal scroll command to shift content in-place.
    fn scroll_terminal(&mut self, plan: &ScrollPlan) -> io::Result<()>;

    /// Begin synchronized output. The terminal buffers all subsequent writes
    /// until `end_sync` and renders them in a single frame.
    fn begin_sync(&mut self) -> io::Result<()>;

    /// End synchronized output and let the terminal render the buffered frame.
    fn end_sync(&mut self) -> io::Result<()>;

    fn flush(&mut self) -> io::Result<()>;
}

/// crossterm-based terminal screen.
pub struct TermScreen {
    stdout: Stdout,
}

impl TermScreen {
    pub fn new() -> io::Result<Self> {
        let mut stdout = io::stdout();
        terminal::enable_raw_mode()?;
        execute!(stdout, terminal::EnterAlternateScreen, cursor::Hide,)?;
        Ok(Self { stdout })
    }
}

impl Drop for TermScreen {
    fn drop(&mut self) {
        let _ = execute!(
            self.stdout,
            EndSynchronizedUpdate,
            cursor::Show,
            terminal::LeaveAlternateScreen,
        );
        let _ = terminal::disable_raw_mode();
    }
}

impl Screen for TermScreen {
    fn size(&self) -> io::Result<(u16, u16)> {
        terminal::size()
    }

    fn poll_event(&mut self, timeout: std::time::Duration) -> io::Result<Option<Event>> {
        if event::poll(timeout)? {
            Ok(Some(event::read()?))
        } else {
            Ok(None)
        }
    }

    fn clear_row(&mut self, screen_y: u16) -> io::Result<()> {
        // Reset SGR attributes before clearing to prevent BCE (Background Color Erase) from
        // filling the row with a stale background color left over from previously written content.
        queue!(
            self.stdout,
            Print("\x1b[0m"),
            cursor::MoveTo(0, screen_y),
            terminal::Clear(ClearType::CurrentLine),
        )
    }

    fn write_at(&mut self, screen_y: u16, text: &str) -> io::Result<()> {
        queue!(self.stdout, cursor::MoveTo(0, screen_y), Print(text),)
    }

    fn scroll_terminal(&mut self, plan: &ScrollPlan) -> io::Result<()> {
        let n = plan.terminal_scroll.get() as u16;
        match plan.direction {
            Direction::Down => {
                queue!(self.stdout, terminal::ScrollUp(n))?;
            }
            Direction::Up => {
                queue!(self.stdout, terminal::ScrollDown(n))?;
            }
        }
        Ok(())
    }

    fn begin_sync(&mut self) -> io::Result<()> {
        queue!(self.stdout, BeginSynchronizedUpdate)
    }

    fn end_sync(&mut self) -> io::Result<()> {
        queue!(self.stdout, EndSynchronizedUpdate)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.stdout.flush()
    }
}
