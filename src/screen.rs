use std::io::{self, Stdout, Write};

use crossterm::{
    cursor,
    event::{self, Event},
    execute, queue,
    style::Print,
    terminal::{self, ClearType},
};
mod highlight;

use crate::document::Document;
use crate::page::Page;
use crate::search::SearchState;
use crate::viewport::{Direction, ScreenRow, ScrollPlan};

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

    /// Restrict which rows are affected by scroll commands (DECSTBM).
    /// `top` and `bottom` are 0-indexed inclusive row bounds.
    fn set_scroll_region(&mut self, top: u16, bottom: u16) -> io::Result<()>;

    /// Reset the scroll region to the full screen.
    fn reset_scroll_region(&mut self) -> io::Result<()>;

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

    fn set_scroll_region(&mut self, top: u16, bottom: u16) -> io::Result<()> {
        // DECSTBM: CSI top ; bottom r (1-indexed)
        queue!(
            self.stdout,
            Print(format!("\x1b[{};{}r", top + 1, bottom + 1))
        )
    }

    fn reset_scroll_region(&mut self) -> io::Result<()> {
        // CSI r with no parameters resets to full screen.
        queue!(self.stdout, Print("\x1b[r"))
    }

    fn flush(&mut self) -> io::Result<()> {
        self.stdout.flush()
    }
}

// ---------------------------------------------------------------------------
// Rendering helpers
// ---------------------------------------------------------------------------

/// Draw screen rows, grouping consecutive rows from the same logical line
/// and writing them as a single continuous string so the terminal treats
/// line-internal wraps as soft wraps.
/// `screen_y` specifies the starting screen row position for drawing.
fn draw_rows_grouped<S: Screen>(
    screen: &mut S,
    doc: &mut Document,
    rows: &[ScreenRow],
    width: usize,
    search: Option<&SearchState>,
    screen_y: usize,
) -> io::Result<()> {
    let mut i = 0;
    while i < rows.len() {
        let line_idx = rows[i].line_index;
        let group_start = i;
        while i < rows.len() && rows[i].line_index == line_idx {
            i += 1;
        }
        // Clear each row in the group
        for j in group_start..i {
            screen.clear_row((j + screen_y) as u16)?;
        }
        // Write the combined text for this group as one continuous piece
        if let Some(line) = doc.line(line_idx) {
            let first_wrap = rows[group_start].wrap_index;
            let last_wrap = rows[i - 1].wrap_index;
            let raw_range = line.wrap_rows_range(width, first_wrap, last_wrap + 1);

            let matches = search.map(|sh| line.find_matches(&sh.query));
            match (search, matches) {
                (Some(search), Some(matches)) if !matches.is_empty() => {
                    let current_match_index = search.current.and_then(|current| {
                        if current.line == line_idx {
                            Some(current.match_index)
                        } else {
                            None
                        }
                    });
                    let positions = highlight::build_highlight_positions(
                        &matches,
                        current_match_index,
                        line.plain_to_raw(),
                        line.raw().len(),
                    );
                    let text =
                        highlight::apply_highlight_to_range(line.raw(), raw_range, &positions);
                    screen.write_at((group_start + screen_y) as u16, &text)?;
                }
                _ => {
                    screen.write_at((group_start + screen_y) as u16, &line.raw()[raw_range])?;
                }
            }
        }
    }
    Ok(())
}

/// Draw the status line, computing its position from the page layout.
pub fn draw_status_line<S: Screen>(screen: &mut S, page: &mut Page) -> io::Result<()> {
    draw_status_line_no_flush(screen, page)?;
    screen.flush()
}

fn draw_status_line_no_flush<S: Screen>(screen: &mut S, page: &mut Page) -> io::Result<()> {
    let header_height = page.resolve_header().len();
    let status_y = (header_height + page.viewport.rows().len()) as u16;
    screen.clear_row(status_y)?;
    screen.write_at(status_y, page.status.render())?;
    Ok(())
}

/// Redraw only the header area (used when section header changes
/// but header height stays the same during incremental scroll).
pub fn redraw_header<S: Screen>(
    screen: &mut S,
    page: &mut Page,
    search: Option<&SearchState>,
) -> io::Result<()> {
    let width = page.viewport.width();
    let header_rows = page.resolve_header();
    if !header_rows.is_empty() {
        draw_rows_grouped(screen, &mut page.doc, &header_rows, width, search, 0)?;
        screen.flush()?;
    }
    Ok(())
}

/// Render a full page (used on initial draw and resize).
pub fn draw_full_page<S: Screen>(
    screen: &mut S,
    page: &mut Page,
    search: Option<&SearchState>,
) -> io::Result<()> {
    let width = page.viewport.width();
    let header_rows = page.resolve_header_synced();
    let header_height = header_rows.len();

    // Draw header rows at the top of the screen.
    if header_height > 0 {
        draw_rows_grouped(screen, &mut page.doc, &header_rows, width, search, 0)?;
    }

    // Draw viewport rows below the header.
    let rows = page.viewport.rows();
    draw_rows_grouped(screen, &mut page.doc, rows, width, search, header_height)?;

    // Clear any rows below content that may have stale content.
    for y in rows.len()..page.viewport.height() {
        screen.clear_row((y + header_height) as u16)?;
    }
    draw_status_line_no_flush(screen, page)?;

    screen.flush()
}

/// Apply a scroll plan with soft-wrap-aware rendering.
///
/// After terminal scroll, we determine the "dirty range": the new rows plus
/// any adjacent existing rows that belong to the same logical line. The dirty
/// range is then redrawn with grouped continuous writes to maintain soft wraps.
///
/// When a header is present, a scroll region is used to keep it in place.
pub fn apply_scroll<S: Screen>(
    screen: &mut S,
    plan: &ScrollPlan,
    page: &mut Page,
    search: Option<&SearchState>,
) -> io::Result<()> {
    let width = page.viewport.width();
    let header_height = page.resolve_header().len();

    // Set scroll region to exclude header and status line.
    let viewport_height = page.viewport.height();
    if header_height > 0 {
        let region_top = header_height as u16;
        let region_bottom = (header_height + viewport_height - 1) as u16;
        screen.set_scroll_region(region_top, region_bottom)?;
    }

    screen.scroll_terminal(plan)?;

    if header_height > 0 {
        screen.reset_scroll_region()?;
    }

    let rows = page.viewport.rows();
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

    draw_rows_grouped(
        screen,
        &mut page.doc,
        &rows[draw_from..draw_to],
        width,
        search,
        header_height + draw_from,
    )?;
    draw_status_line_no_flush(screen, page)?;

    screen.flush()
}
