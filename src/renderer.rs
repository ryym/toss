mod highlight;

use std::{borrow::Cow, collections::HashSet, io, mem, ops::Range};

use crossterm::event::Event;

use crate::{
    document::Document,
    line::{MatchPosition, Row},
    pager::{PageSnapshot, PageUpdate},
    screen::{Direction, Screen, Scroll},
    search::SearchState,
};

struct SearchStateRef {
    query: String,
    current: Option<MatchPosition>,
}

impl PartialEq<SearchState> for SearchStateRef {
    fn eq(&self, other: &SearchState) -> bool {
        self.query == other.query.as_str() && self.current == other.current
    }
}

/// Renders a [`PageSnapshot`] to the terminal via a [`Screen`].
///
/// `Renderer` is the bridge between [`crate::pager::Pager`], which manages the page state,
/// and [`Screen`], which abstracts terminal writes. Its sole job is to apply the current
/// page state to the screen; it never modifies the page state itself.
///
/// It also keeps a small amount of state from the previous render so that it can
/// re-apply or clear search highlights on rows that a terminal scroll would otherwise leave
/// untouched.
pub struct Renderer<S: Screen> {
    screen: S,
    last_search: Option<SearchStateRef>,
    last_highlight_lines: HashSet<usize>,
    current_highlight_lines: HashSet<usize>,
}

impl<S: Screen> Renderer<S> {
    pub fn new(screen: S) -> Self {
        Self {
            screen,
            last_search: None,
            last_highlight_lines: HashSet::new(),
            current_highlight_lines: HashSet::new(),
        }
    }

    pub fn into_screen(self) -> S {
        self.screen
    }

    pub fn poll_event(&mut self, timeout: std::time::Duration) -> io::Result<Option<Event>> {
        self.screen.poll_event(timeout)
    }

    /// Apply the given [`PageSnapshot`] to the screen.
    /// To keep updates smooth and avoid unnecessary flicker, picks a redraw strategy
    /// based on the [`PageUpdate`] reported by the snapshot:
    ///
    /// - [`PageUpdate::None`]: only the status line is rewritten.
    /// - [`PageUpdate::Full`]: the entire page (headers, content, status line) is redrawn.
    /// - [`PageUpdate::Partial`]: only rows affected by state changes are redrawn. when a
    ///   [`Scroll`] is provided, the terminal is scrolled so the existing rows are preserved.
    pub fn render(&mut self, doc: &mut Document, page: PageSnapshot) -> io::Result<()> {
        log::debug!("render: {:?}", page.last_update);
        let result = match page.last_update {
            PageUpdate::None => self.render_status_line(&page),
            PageUpdate::Full => self.render_full_page(doc, &page),
            PageUpdate::Partial(scroll) => self.render_partial(doc, &page, scroll),
        };
        self.store_page_state(page.search);
        result
    }

    fn store_page_state(&mut self, search: Option<&SearchState>) {
        self.last_search = search.map(|s| SearchStateRef {
            query: s.query.as_str().to_string(),
            current: s.current.clone(),
        });
        self.last_highlight_lines = mem::take(&mut self.current_highlight_lines);
    }

    fn render_status_line(&mut self, page: &PageSnapshot) -> io::Result<()> {
        self.redraw_status_line(page)?;
        self.screen.flush()
    }

    fn redraw_status_line(&mut self, page: &PageSnapshot) -> io::Result<()> {
        let status_y = page.viewport_height() as u16;
        self.screen.clear_row(status_y)?;
        self.screen.write_at(status_y, &page.status_line)?;
        Ok(())
    }

    fn render_full_page(&mut self, doc: &mut Document, page: &PageSnapshot) -> io::Result<()> {
        self.screen.begin_sync()?;

        self.draw_rows(doc, page.header, page.search, 0)?;
        self.draw_rows(doc, page.heading, page.search, page.header.len())?;
        self.draw_rows(doc, page.content, page.search, page.total_header_height())?;
        self.clear_row_range(0, page.viewport_height()..page.height)?;
        self.redraw_status_line(page)?;

        self.screen.end_sync()?;
        self.screen.flush()
    }

    fn render_partial(
        &mut self,
        doc: &mut Document,
        page: &PageSnapshot,
        scroll: Option<Scroll>,
    ) -> io::Result<()> {
        self.screen.begin_sync()?;

        let header_height = page.total_header_height();
        let ranges = compute_scroll_redraw_ranges(page.content, scroll.as_ref());
        log::debug!("render partial: new_rows_range={:?}", ranges);

        // Scroll the terminal and draw newly appeared rows.
        if let Some(scroll) = &scroll {
            self.screen.scroll_terminal(scroll)?;
            let screen_y = header_height + ranges.new_rows.start;
            self.draw_rows(doc, &page.content[ranges.new_rows], page.search, screen_y)?;
        }

        // If the search state has changed, refresh other rows as well.
        let is_search_same = match (&self.last_search, page.search) {
            (Some(prev), Some(current)) => prev == current,
            (None, None) => true,
            _ => false,
        };
        if !is_search_same {
            let screen_y = header_height + ranges.remaining.start;
            self.refresh_rows(doc, &page.content[ranges.remaining], page.search, screen_y)?;
        }

        // Refresh headers and the status line.
        self.draw_rows(doc, page.header, page.search, 0)?;
        self.draw_rows(doc, page.heading, page.search, page.header.len())?;
        self.redraw_status_line(page)?;

        self.screen.end_sync()?;
        self.screen.flush()
    }

    /// Draw rows, grouping consecutive rows from the same logical line and writing them as a
    /// single continuous string so that the terminal treats line-internal wraps as soft wraps.
    /// `screen_y` specifies the starting screen row position for drawing.
    fn draw_rows(
        &mut self,
        doc: &mut Document,
        rows: &[Row],
        search: Option<&SearchState>,
        screen_y: usize,
    ) -> io::Result<()> {
        let mut i = 0;
        while i < rows.len() {
            let line_idx = rows[i].line_index();
            let start = i;
            while i < rows.len() && rows[i].line_index() == line_idx {
                i += 1;
            }
            self.clear_row_range(screen_y, start..i)?;
            // Write the combined text for this group as one continuous piece
            if let Some(line) = doc.line(line_idx) {
                let raw_range = rows[start].raw_range().start..rows[i - 1].raw_range().end;
                let text = highlight::apply_highlight_if_matches(search, line, raw_range);
                if matches!(&text, Cow::Owned(_)) {
                    self.current_highlight_lines.insert(line_idx);
                }
                self.screen.write_at((start + screen_y) as u16, &text)?;
            }
        }
        Ok(())
    }

    /// Similar to [`Self::draw_rows`], but redraw rows only if the rows on screen are stale.
    /// Reduce screen flickering by skipping redraws whenever possible.
    fn refresh_rows(
        &mut self,
        doc: &mut Document,
        rows: &[Row],
        search: Option<&SearchState>,
        screen_y: usize,
    ) -> io::Result<()> {
        let mut i = 0;
        while i < rows.len() {
            let line_idx = rows[i].line_index();
            let start = i;
            while i < rows.len() && rows[i].line_index() == line_idx {
                i += 1;
            }
            // Write the combined text for this group as one continuous piece
            if let Some(line) = doc.line(line_idx) {
                let raw_range = rows[start].raw_range().start..rows[i - 1].raw_range().end;
                match highlight::apply_highlight_if_matches(search, line, raw_range) {
                    Cow::Owned(text) => {
                        self.current_highlight_lines.insert(line_idx);
                        self.clear_row_range(screen_y, start..i)?;
                        self.screen.write_at((start + screen_y) as u16, &text)?;
                    }
                    Cow::Borrowed(text) => {
                        // Skip redraw only if the line has no highlights both before and this time.
                        if !self.last_highlight_lines.contains(&line_idx) {
                            continue;
                        }
                        self.clear_row_range(screen_y, start..i)?;
                        self.screen.write_at((start + screen_y) as u16, text)?;
                    }
                };
            }
        }
        Ok(())
    }

    fn clear_row_range(&mut self, base: usize, range: Range<usize>) -> io::Result<()> {
        for i in range {
            self.screen.clear_row((base + i) as u16)?;
        }
        Ok(())
    }
}

#[derive(Debug)]
struct ScrollRedrawRanges {
    /// A range of newly appeared rows in the screen by scrolling.
    new_rows: Range<usize>,
    /// A range of rows other than [`Self::new_rows`]; rows existing before scroll.
    remaining: Range<usize>,
}

/// Compute the range of rows that need redrawing after a scroll as well as the remaining rows.
fn compute_scroll_redraw_ranges(rows: &[Row], scroll: Option<&Scroll>) -> ScrollRedrawRanges {
    let (from, to) = scroll_dirty_range(rows, scroll);
    let remaining = if from == 0 { to..rows.len() } else { 0..from };
    ScrollRedrawRanges {
        new_rows: from..to,
        remaining,
    }
}

/// After terminal scroll shifts content, `scroll_rows` new rows appear at one edge.
/// This function returns the range extended to include adjacent existing
/// rows from the same logical line, so soft-wrap groups are drawn correctly.
fn scroll_dirty_range(rows: &[Row], scroll: Option<&Scroll>) -> (usize, usize) {
    let Some(scroll) = scroll else {
        return (0, 0);
    };
    let num_rows = scroll.num_rows.get();
    let len = rows.len();
    match scroll.direction {
        Direction::Down => {
            let new_start = len.saturating_sub(num_rows);
            let mut from = new_start;
            while from > 0 && rows[from - 1].line_index() == rows[new_start].line_index() {
                from -= 1;
            }
            (from, len)
        }
        Direction::Up => {
            let new_end = num_rows.min(len);
            let mut to = new_end;
            while to < len && rows[to].line_index() == rows[new_end - 1].line_index() {
                to += 1;
            }
            (0, to)
        }
    }
}
