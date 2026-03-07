use std::io::{self, Stdout, Write};

use crossterm::{
    cursor, execute, queue,
    event::{self, Event},
    style::Print,
    terminal::{self, ClearType},
};

use crate::screen_state::{Direction, ScrollPlan, ScreenRow};
use crate::document::Document;

/// Abstract terminal operations for rendering and input.
pub trait Screen {
    fn size(&self) -> io::Result<(u16, u16)>;
    fn poll_event(&self, timeout: std::time::Duration) -> io::Result<Option<Event>>;
    fn draw_row(&mut self, screen_y: u16, text: &str) -> io::Result<()>;
    fn scroll_terminal(&mut self, plan: &ScrollPlan) -> io::Result<()>;
    fn clear_and_flush(&mut self) -> io::Result<()>;
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
        execute!(
            stdout,
            terminal::EnterAlternateScreen,
            cursor::Hide,
        )?;
        Ok(Self { stdout })
    }
}

impl Drop for TermScreen {
    fn drop(&mut self) {
        let _ = execute!(
            self.stdout,
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

    fn poll_event(&self, timeout: std::time::Duration) -> io::Result<Option<Event>> {
        if event::poll(timeout)? {
            Ok(Some(event::read()?))
        } else {
            Ok(None)
        }
    }

    fn draw_row(&mut self, screen_y: u16, text: &str) -> io::Result<()> {
        queue!(
            self.stdout,
            cursor::MoveTo(0, screen_y),
            terminal::Clear(ClearType::CurrentLine),
            Print(text),
        )
    }

    fn scroll_terminal(&mut self, plan: &ScrollPlan) -> io::Result<()> {
        if plan.terminal_scroll == 0 {
            return Ok(());
        }
        let n = plan.terminal_scroll as u16;
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

    fn clear_and_flush(&mut self) -> io::Result<()> {
        execute!(
            self.stdout,
            terminal::Clear(ClearType::All),
        )
    }

    fn flush(&mut self) -> io::Result<()> {
        self.stdout.flush()
    }
}

/// Render a full page (used on initial draw and resize).
pub fn draw_full_page<S: Screen>(
    screen: &mut S,
    doc: &Document,
    rows: &[ScreenRow],
    width: usize,
) -> io::Result<()> {
    screen.clear_and_flush()?;
    for (i, row) in rows.iter().enumerate() {
        if let Some(line) = doc.line(row.line_index) {
            let text = line.wrap_row_text(width, row.wrap_index);
            screen.draw_row(i as u16, text)?;
        }
    }
    screen.flush()
}

/// Apply a scroll plan: scroll the terminal, then draw only new rows.
pub fn apply_scroll<S: Screen>(
    screen: &mut S,
    doc: &Document,
    plan: &ScrollPlan,
    total_height: usize,
    width: usize,
) -> io::Result<()> {
    if plan.terminal_scroll == 0 {
        return Ok(());
    }

    screen.scroll_terminal(plan)?;

    let new_count = plan.new_rows.len();
    for (i, row) in plan.new_rows.iter().enumerate() {
        let screen_y = match plan.direction {
            Direction::Down => (total_height - new_count + i) as u16,
            Direction::Up => i as u16,
        };
        if let Some(line) = doc.line(row.line_index) {
            let text = line.wrap_row_text(width, row.wrap_index);
            screen.draw_row(screen_y, text)?;
        }
    }
    screen.flush()
}
