use std::io::{self, BufWriter, Stdout, Write};

use crossterm::{
    cursor,
    event::{self, Event},
    execute, queue,
    style::Print,
    terminal::{self, BeginSynchronizedUpdate, ClearType, EndSynchronizedUpdate},
};

use super::{Direction, Screen, ScreenSize, Scroll};

/// Convert an internal `usize` coordinate to the `u16` crossterm expects.
/// Coordinates never exceed the terminal size (well within `u16`), but clamp
/// defensively so an out-of-range value targets the last row instead of wrapping.
fn to_u16(value: usize) -> u16 {
    u16::try_from(value).unwrap_or(u16::MAX)
}

/// crossterm-based terminal screen.
pub struct TermScreen {
    stdout: BufWriter<Stdout>,
}

impl TermScreen {
    pub fn new() -> io::Result<Self> {
        terminal::enable_raw_mode()?;
        let stdout = io::stdout();
        let mut stdout = BufWriter::with_capacity(16384, stdout);
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
    fn size(&self) -> io::Result<ScreenSize> {
        let (w, h) = terminal::size()?;
        Ok(ScreenSize::new(w, h))
    }

    fn poll_event(&mut self, timeout: std::time::Duration) -> io::Result<Option<Event>> {
        if event::poll(timeout)? {
            Ok(Some(event::read()?))
        } else {
            Ok(None)
        }
    }

    fn clear_row(&mut self, screen_y: usize) -> io::Result<()> {
        // Reset SGR attributes before clearing to prevent BCE (Background Color Erase) from
        // filling the row with a stale background color left over from previously written content.
        queue!(
            self.stdout,
            Print("\x1b[0m"),
            cursor::MoveTo(0, to_u16(screen_y)),
            terminal::Clear(ClearType::CurrentLine),
        )
    }

    fn write_at(&mut self, screen_y: usize, text: &str) -> io::Result<()> {
        queue!(
            self.stdout,
            cursor::MoveTo(0, to_u16(screen_y)),
            Print(text),
        )
    }

    fn scroll_terminal(&mut self, scroll: &Scroll) -> io::Result<()> {
        let n = to_u16(scroll.num_rows.get());
        match scroll.direction {
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
