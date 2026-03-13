use std::io::{self, Stdout, Write};

use crossterm::{
    cursor,
    event::{self, Event},
    execute, queue,
    style::Print,
    terminal::{self, ClearType},
};

use crate::document::Document;
use crate::screen_state::{Direction, ScreenRow, ScreenState, ScrollPlan};
use crate::status_line::StatusLine;

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

    fn scroll_terminal(&mut self, plan: &ScrollPlan) -> io::Result<()>;
    fn clear_all(&mut self) -> io::Result<()>;
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
        let _ = execute!(self.stdout, cursor::Show, terminal::LeaveAlternateScreen,);
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
        queue!(
            self.stdout,
            cursor::MoveTo(0, screen_y),
            terminal::Clear(ClearType::CurrentLine),
        )
    }

    fn write_at(&mut self, screen_y: u16, text: &str) -> io::Result<()> {
        queue!(self.stdout, cursor::MoveTo(0, screen_y), Print(text),)
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

    fn clear_all(&mut self) -> io::Result<()> {
        queue!(self.stdout, terminal::Clear(ClearType::All),)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.stdout.flush()
    }
}

// ---------------------------------------------------------------------------
// Rendering helpers
// ---------------------------------------------------------------------------

/// Draw a range of screen rows, grouping consecutive rows from the same
/// logical line and writing them as a single continuous string so the
/// terminal treats line-internal wraps as soft wraps.
fn draw_rows_grouped<S: Screen>(
    screen: &mut S,
    doc: &mut Document,
    all_rows: &[ScreenRow],
    from: usize,
    to: usize,
    width: usize,
) -> io::Result<()> {
    let mut i = from;
    while i < to {
        let line_idx = all_rows[i].line_index;
        let group_start = i;
        while i < to && all_rows[i].line_index == line_idx {
            i += 1;
        }
        // Clear each row in the group
        for j in group_start..i {
            screen.clear_row(j as u16)?;
        }
        // Write the combined text for this group as one continuous piece
        if let Some(line) = doc.line(line_idx) {
            let text: String = (group_start..i)
                .map(|j| line.wrap_row_text(width, all_rows[j].wrap_index))
                .collect();
            screen.write_at(group_start as u16, &text)?;
        }
    }
    Ok(())
}

/// Draw the status line at the given screen row.
pub fn draw_status_line<S: Screen>(
    screen: &mut S,
    status: &StatusLine,
    screen_y: u16,
) -> io::Result<()> {
    screen.clear_row(screen_y)?;
    screen.write_at(screen_y, status.render())
}

/// Render a full page (used on initial draw and resize).
pub fn draw_full_page<S: Screen>(
    screen: &mut S,
    doc: &mut Document,
    state: &ScreenState,
    status: &StatusLine,
) -> io::Result<()> {
    let rows = state.rows();
    screen.clear_all()?;
    draw_rows_grouped(screen, doc, rows, 0, rows.len(), state.width())?;
    draw_status_line(screen, status, rows.len() as u16)?;
    screen.flush()
}

/// Apply a scroll plan with soft-wrap-aware rendering.
///
/// After terminal scroll, we determine the "dirty range": the new rows plus
/// any adjacent existing rows that belong to the same logical line. The dirty
/// range is then redrawn with grouped continuous writes to maintain soft wraps.
pub fn apply_scroll<S: Screen>(
    screen: &mut S,
    doc: &mut Document,
    plan: &ScrollPlan,
    state: &ScreenState,
    status: &StatusLine,
) -> io::Result<()> {
    if plan.terminal_scroll == 0 {
        return Ok(());
    }

    screen.scroll_terminal(plan)?;

    let rows = state.rows();
    let content_height = rows.len();
    let n_new = plan.new_rows.len();

    let (draw_from, draw_to) = match plan.direction {
        Direction::Down => {
            let new_start = content_height - n_new;
            // Extend backwards: include existing rows of the same line
            let mut from = new_start;
            while from > 0 && rows[from - 1].line_index == rows[new_start].line_index {
                from -= 1;
            }
            (from, content_height)
        }
        Direction::Up => {
            let new_end = n_new;
            // Extend forwards: include existing rows of the same line
            let mut to = new_end;
            while to < content_height && rows[to].line_index == rows[new_end - 1].line_index {
                to += 1;
            }
            (0, to)
        }
    };

    draw_rows_grouped(screen, doc, rows, draw_from, draw_to, state.width())?;
    draw_status_line(screen, status, rows.len() as u16)?;
    screen.flush()
}
