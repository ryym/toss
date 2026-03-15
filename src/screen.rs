use std::io::{self, Stdout, Write};

use crossterm::{
    cursor,
    event::{self, Event},
    execute, queue,
    style::Print,
    terminal::{self, ClearType},
};
use regex::Regex;

use crate::document::Document;
mod highlight;

use crate::page::Page;
use crate::search::MatchPosition;
use crate::status_line::StatusLine;
use crate::viewport::{Direction, ScreenRow, ScrollPlan};
use highlight::HighlightStyle;

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

/// Search context for highlighting matches during rendering.
pub struct SearchHighlight<'a> {
    pub query: &'a Regex,
    /// Current match position. The current match uses reverse video,
    /// other matches use dim reverse.
    pub current: Option<MatchPosition>,
}

/// Draw a range of screen rows, grouping consecutive rows from the same
/// logical line and writing them as a single continuous string so the
/// terminal treats line-internal wraps as soft wraps.
///
/// `screen_y_offset` shifts all row positions by the given amount
/// (used to render viewport rows below the header area).
fn draw_rows_grouped<S: Screen>(
    screen: &mut S,
    doc: &mut Document,
    all_rows: &[ScreenRow],
    from: usize,
    to: usize,
    width: usize,
    search: Option<&SearchHighlight<'_>>,
    screen_y_offset: usize,
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
            screen.clear_row((j + screen_y_offset) as u16)?;
        }
        // Write the combined text for this group as one continuous piece
        if let Some(line) = doc.line(line_idx) {
            let text = match search {
                Some(sh) => {
                    let matches = line.find_matches(sh.query);
                    if matches.is_empty() {
                        // No matches: use original text as-is
                        (group_start..i)
                            .map(|j| {
                                line.wrap_row_text(width, all_rows[j].wrap_index)
                                    .to_string()
                            })
                            .collect::<String>()
                    } else {
                        let styles: Vec<HighlightStyle> = matches
                            .iter()
                            .enumerate()
                            .map(|(mi, _)| {
                                let is_current = sh
                                    .current
                                    .is_some_and(|c| c.line == line_idx && c.match_index == mi);
                                if is_current {
                                    HighlightStyle::Reverse
                                } else {
                                    HighlightStyle::DimReverse
                                }
                            })
                            .collect();
                        let positions = highlight::build_highlight_positions(
                            &matches,
                            &styles,
                            line.plain_to_raw(),
                            line.raw().len(),
                        );
                        (group_start..i)
                            .map(|j| {
                                let range = line.wrap_row_range(width, all_rows[j].wrap_index);
                                highlight::apply_highlight_to_range(line.raw(), range, &positions)
                            })
                            .collect::<String>()
                    }
                }
                None => (group_start..i)
                    .map(|j| {
                        line.wrap_row_text(width, all_rows[j].wrap_index)
                            .to_string()
                    })
                    .collect::<String>(),
            };
            screen.write_at((group_start + screen_y_offset) as u16, &text)?;
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
    page: &mut Page,
    header_rows: &[ScreenRow],
    search: Option<&SearchHighlight<'_>>,
) -> io::Result<()> {
    let header_height = header_rows.len();
    let width = page.viewport.width();

    // Draw header rows at the top of the screen.
    if header_height > 0 {
        draw_rows_grouped(
            screen,
            &mut page.doc,
            header_rows,
            0,
            header_height,
            width,
            search,
            0,
        )?;
    }

    // Draw viewport rows below the header.
    let rows = page.viewport.rows();
    draw_rows_grouped(
        screen,
        &mut page.doc,
        rows,
        0,
        rows.len(),
        width,
        search,
        header_height,
    )?;

    // Clear any rows below content that may have stale content.
    for y in rows.len()..page.viewport.height() {
        screen.clear_row((y + header_height) as u16)?;
    }

    let status_y = (header_height + rows.len()) as u16;
    draw_status_line(screen, &page.status, status_y)?;
    screen.flush()
}

/// Apply a scroll plan with soft-wrap-aware rendering.
///
/// After terminal scroll, we determine the "dirty range": the new rows plus
/// any adjacent existing rows that belong to the same logical line. The dirty
/// range is then redrawn with grouped continuous writes to maintain soft wraps.
///
/// `header_height` is the number of screen rows occupied by the header.
/// When non-zero, a scroll region is used to keep the header in place.
pub fn apply_scroll<S: Screen>(
    screen: &mut S,
    plan: &ScrollPlan,
    page: &mut Page,
    header_height: usize,
    search: Option<&SearchHighlight<'_>>,
) -> io::Result<()> {
    if plan.terminal_scroll == 0 {
        return Ok(());
    }

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
    let width = page.viewport.width();
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
        rows,
        draw_from,
        draw_to,
        width,
        search,
        header_height,
    )?;
    let status_y = (header_height + content_height) as u16;
    draw_status_line(screen, &page.status, status_y)?;
    screen.flush()
}
