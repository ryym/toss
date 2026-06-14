use std::mem;

use regex::Regex;

use crate::{
    document::Document,
    line::{MatchPosition, Row},
    line_editor::{LineEdit, LineEditor},
    options::Options,
    pager::{header::Header, heading::Heading, viewport::Viewport},
    screen::{Direction, ScreenSize, Scroll},
    search::{self, SearchDirection, SearchFrom, SearchState},
};

mod header;
mod heading;
mod rows;
mod viewport;

#[derive(Debug, Clone, Copy)]
struct ViewportSize {
    width: usize,
    height: usize,
}

impl ViewportSize {
    fn new(screen_width: usize, screen_height: usize) -> Self {
        Self {
            width: screen_width,
            height: screen_height - 1, // Reserve the status line area
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

#[derive(Debug, Clone, Copy)]
pub enum PageUpdate {
    Full,
    Partial(Option<Scroll>),
    StatusOnly,
}

#[derive(Debug)]
pub struct PageSnapshot<'pager> {
    pub header: &'pager [Row],
    pub heading: &'pager [Row],
    pub content: &'pager [Row],
    pub height: usize,
    pub search: Option<&'pager SearchState>,
    pub status_line: String,
}

impl<'pager> PageSnapshot<'pager> {
    pub fn total_header_height(&self) -> usize {
        self.header.len() + self.heading.len()
    }

    pub fn viewport_height(&self) -> usize {
        self.total_header_height() + self.content.len()
    }
}

#[derive(Default)]
pub enum PagerMode {
    #[default]
    View,
    SearchInput(SearchInputMode),
}

pub struct SearchInputMode {
    direction: SearchDirection,
    editor: LineEditor,
    /// Top line where search started, for searching and restoring on cancel.
    start_line_index: usize,
    /// Live search state before finalizing a search query.
    draft: Option<SearchState>,
}

/// Centrally manages the pagination state.
/// [`Pager`] reads the rows that fit in the display area from [`Document`] and shows them
/// together with the status line. The whole display area is called the page, and the part
/// that shows rows of [`Document`] lines in particular is called the viewport.
/// Depending on the configuration, a global header and a heading may be pinned at
/// the top of the viewport.
///
/// Internally the following structs manage rows displayed in sticky area and viewport:
/// - Sticky area
///     - Global header: [`Header`]
///     - Heading: [`Heading`]
/// - Viewport: [`Viewport`]
///
/// [`Viewport`] is unaware of sticky rows and just holds a specific range of [`Document`]
/// as directed by [`Pager`]. The header rows managed by [`Header`] and [`Heading`]
/// are rendered as if overlaid on top of [`Viewport`].
/// With this overlay approach, [`Viewport`] can manage its rows independently,
/// without being affected by header content or height.
/// The role of [`Pager`] is to maintain this overlay correctly while applying the requested
/// operations to update the page state.
/// [`Pager`] only holds the state but does not write anything to the screen itself.
pub struct Pager {
    doc: Document,
    mode: PagerMode,
    header: Header,
    heading: Heading,
    viewport: Viewport,
    search: Option<SearchState>,
}

impl Pager {
    pub fn new(mut doc: Document, options: Options, screen_size: ScreenSize) -> Self {
        let size = ViewportSize::new(screen_size.width(), screen_size.height());
        let header = Header::new(&mut doc, &size, options.header);
        let mut heading = Heading::new(options.heading, &size, header.height());
        heading.resolve(&mut doc, 0);
        let viewport = Viewport::new(&mut doc, size);
        Self {
            doc,
            mode: PagerMode::View,
            header,
            heading,
            viewport,
            search: None,
        }
    }

    pub fn mode(&self) -> &PagerMode {
        &self.mode
    }

    pub fn doc_mut(&mut self) -> &mut Document {
        &mut self.doc
    }

    fn status_line(&self) -> String {
        match &self.mode {
            PagerMode::View => ":".to_string(),
            PagerMode::SearchInput(search) => format!(
                "{}{}",
                search.direction.prompt(),
                search.editor.input_with_cursor()
            ),
        }
    }

    pub fn snapshot<'pager>(&'pager mut self) -> (PageSnapshot<'pager>, &'pager mut Document) {
        let search = match &self.mode {
            PagerMode::SearchInput(search) => search.draft.as_ref().or(self.search.as_ref()),
            _ => self.search.as_ref(),
        };
        let snapshot = PageSnapshot {
            header: self.header.rows(),
            heading: self.heading.rows(),
            content: &self.viewport.rows()[self.total_header_height()..],
            height: self.viewport.size().height,
            status_line: self.status_line(),
            search,
        };
        (snapshot, &mut self.doc)
    }

    fn total_header_height(&self) -> usize {
        self.header.height() + self.heading.height()
    }

    /// Returns the height of the display area (the number of rows) excluding the header region.
    pub fn content_height(&self) -> usize {
        self.viewport.rows().len() - self.total_header_height()
    }

    /// Returns the rows that form a contiguous range within the viewport.
    /// When sticky area exists, it is included only if its region and the content region
    /// are adjacent in the document; otherwise they are excluded.
    /// For example, if the heading shows lines 3-5 of [`Document`] and the content
    /// shows lines 6-30, the rows for lines 3-30 are returned.
    /// If a global header also exists at lines 1-2, the global header is included as well.
    /// However, if the content starts at line 7 or later (not adjacent), only the content rows
    /// are returned.
    fn contiguous_rows(&self) -> &[Row] {
        &self.viewport.rows()[self.contiguous_top_row_index()..]
    }

    fn contiguous_top_row_index(&self) -> usize {
        let rows = self.viewport.rows();
        if let Some(row) = self.header.rows().first()
            && row == &rows[0]
        {
            return 0;
        }
        if let Some(row) = self.heading.rows().first()
            && row.line_index() == rows[self.header.rows().len()].line_index()
        {
            return self.header.rows().len();
        }
        self.total_header_height()
    }

    /// Whether the entire page fits within the specified `height`.
    pub fn fits_within(&mut self, height: usize) -> bool {
        let mut total_rows = 0;
        for i in 0..self.doc.line_count() {
            if let Some(line) = self.doc.line(i) {
                total_rows += line.row_count(self.viewport.size().width());
                if total_rows > height {
                    return false;
                }
            }
        }
        true
    }

    /// Drain pending streamed input and reflect it in the page.
    /// Returns a [`PageUpdate`] only when the visible page changed.
    ///
    /// While the first screen is still filling in (the viewport has not yet
    /// reached its full height), newly arrived lines are appended from the top
    /// anchor and the headers are rebuilt. Once the viewport is full, appended
    /// tail lines stay below the fold and become visible on scroll, so no
    /// redraw is needed here.
    pub fn pump_input(&mut self) -> Option<PageUpdate> {
        let result = self.doc.pump();
        if !result.grew {
            return None;
        }
        if self.viewport.rows().len() >= self.viewport.size().height() {
            return None;
        }
        let size = *self.viewport.size();
        self.header.resize(&mut self.doc, &size);
        self.heading
            .resize(&mut self.doc, &size, self.header.height());
        self.heading.resolve(&mut self.doc, 0);
        self.viewport.refill_from_top(&mut self.doc);
        Some(PageUpdate::Full)
    }

    /// Resize the page to fit the new dimensions.
    pub fn resize(&mut self, screen_width: usize, screen_height: usize) -> PageUpdate {
        let size = ViewportSize::new(screen_width, screen_height);
        self.header.resize(&mut self.doc, &size);
        self.heading
            .resize(&mut self.doc, &size, self.header.height());
        self.viewport.resize(&mut self.doc, size);
        PageUpdate::Full
    }

    /// Move the page so that the specified line comes to the top.
    /// - If the specified line is within the global header, jump to the start of the document.
    /// - If the specified line is within any heading, move so that it comes to the top.
    /// - Otherwise, move so that the specified line comes right after the headers.
    pub fn jump_to(&mut self, mut line_index: usize) -> PageUpdate {
        if self.header.contains(line_index) {
            line_index = 0;
        }

        let jump_distance = JumpDistance::from(&self.viewport);

        self.heading.resolve(&mut self.doc, line_index);
        let jump_offset = if self.heading.contains(line_index) {
            self.header.height()
        } else {
            self.total_header_height()
        };
        self.viewport
            .jump_to(&mut self.doc, line_index, jump_offset);

        jump_distance.compute(&self.viewport)
    }

    /// Jump to the end of the document so that the last line is at the bottom.
    /// For streamed input this jumps to the currently known end (non-blocking);
    /// lines still arriving become reachable as they are pumped in.
    pub fn jump_to_end(&mut self) -> PageUpdate {
        self.doc.pump();
        self.viewport.jump_to_end(&mut self.doc);

        let top_line_index = self.viewport.rows()[0].line_index();
        self.heading.resolve(&mut self.doc, top_line_index);
        self.push_up_heading_if_needed();

        PageUpdate::Full
    }

    /// Move the page so that the specified line is fully shown with its last row at the bottom.
    /// Unlike [`Self::jump_to`], which anchors the line at the top, this anchors the whole line
    /// at the bottom so that wherever a match sits within the line it stays visible.
    fn jump_to_bottom(&mut self, line_index: usize) -> PageUpdate {
        let jump_distance = JumpDistance::from(&self.viewport);

        // Offset the line so its last wrap row lands on the bottom (i.e. the whole line is shown).
        let width = self.viewport.size().width();
        let row_count = self
            .doc
            .line(line_index)
            .map(|l| l.row_count(width))
            .unwrap_or(1);
        let row_offset = self.viewport.size().height.saturating_sub(row_count);
        self.viewport.jump_to(&mut self.doc, line_index, row_offset);

        // Re-resolve the heading for the new top line (mirrors jump_to_end).
        let top_line_index = self.viewport.rows()[0].line_index();
        self.heading.resolve(&mut self.doc, top_line_index);
        self.push_up_heading_if_needed();

        jump_distance.compute(&self.viewport)
    }

    /// Scroll by the given number of rows (positive = down, negative = up).
    /// Returns the resulting [`PageUpdate`] when any rows were actually scrolled.
    /// Returns `None` when there is no room to scroll.
    pub fn scroll(&mut self, num_rows: i32) -> Option<PageUpdate> {
        if num_rows.unsigned_abs() as usize > self.viewport.size().height {
            panic!("scroll rows too big");
        }

        let actual_scroll_rows = if num_rows < 0 {
            self.scroll_up((-num_rows) as usize)
        } else if num_rows > 0 {
            self.scroll_down(num_rows as usize)
        } else {
            0
        };
        if actual_scroll_rows == 0 {
            return None;
        }
        let direction = if num_rows < 0 {
            Direction::Up
        } else {
            Direction::Down
        };
        Some(PageUpdate::Partial(Scroll::new(
            direction,
            actual_scroll_rows,
        )))
    }

    fn scroll_up(&mut self, num_rows: usize) -> usize {
        let rows_scrolled = self.viewport.scroll_up(&mut self.doc, num_rows);

        // Check the heading status to update it as needed.
        let heading_start = match self.heading.start_line_index() {
            Some(idx) => idx,
            // If there is no current heading, scrolling upward cannot newly reveal one, so do nothing.
            None => return rows_scrolled,
        };

        // If the new top row is above the current heading, search for a heading above it.
        let top_line = self.viewport.rows()[self.header.height()].line_index();
        if top_line < heading_start {
            self.heading.resolve(&mut self.doc, top_line);
        }
        self.push_up_heading_if_needed();

        rows_scrolled
    }

    fn scroll_down(&mut self, num_rows: usize) -> usize {
        let prev_top_line = self.viewport.rows()[self.header.height()].line_index();
        let rows_scrolled = self.viewport.scroll_down(&mut self.doc, num_rows);
        let top_line = self.viewport.rows()[self.header.height()].line_index();

        // If a new heading exists within the moved range, replace the current one with it.
        self.heading
            .resolve_if_found(&mut self.doc, prev_top_line..(top_line + 1));
        self.push_up_heading_if_needed();

        rows_scrolled
    }

    /// Look for another heading underneath the current heading overlay,
    /// and if one is found (i.e. a section transition is in progress), adjust the offset of
    /// the current section so that the new heading becomes visible.
    fn push_up_heading_if_needed(&mut self) {
        let current_start_line = match self.heading.start_line_index() {
            Some(i) => i,
            None => return,
        };

        let overlay_height = self.header.height() + self.heading.full_height();
        let mut other_section_start = overlay_height;
        let rows_under_heading = self
            .viewport
            .rows()
            .iter()
            .enumerate()
            .take(overlay_height)
            .skip(self.header.height());
        for (i, row) in rows_under_heading {
            if row.wrap_index() != 0 || row.line_index() == current_start_line {
                continue;
            }
            if self
                .heading
                .is_heading_start(&mut self.doc, row.line_index())
            {
                other_section_start = i;
                break;
            }
        }
        let push_up = overlay_height.saturating_sub(other_section_start);
        self.heading.push_up(push_up);
    }

    pub fn has_search_input(&self) -> bool {
        match &self.mode {
            PagerMode::SearchInput(mode) => !mode.editor.is_empty(),
            _ => false,
        }
    }

    pub fn start_search_input(&mut self, direction: SearchDirection) -> PageUpdate {
        // Start searching from the top line of the contiguous rows.
        // The first purpose is why it needs to be a first line of contiguous rows that
        // may include header rows. When the header line is not overlaid, include it in the
        // search range. If a match is found, place the initial cursor position within the header.
        // Excluding the header would cause unnatural behavior where the cursor starts in the content
        // during preview and only jumps to the header via n/N after the query is submitted.
        let start_line_index = self.contiguous_rows()[0].line_index();
        let editor = LineEditor::new();
        self.mode = PagerMode::SearchInput(SearchInputMode {
            direction,
            editor,
            start_line_index,
            draft: None,
        });
        PageUpdate::StatusOnly
    }

    /// Commit the current search input.
    pub fn submit_search(&mut self) -> PageUpdate {
        if let PagerMode::SearchInput(mut mode) = mem::take(&mut self.mode)
            && let Some(draft) = mode.draft.take()
        {
            log::debug!("Submit search: query={:?}", draft.query.as_str());
            self.search = Some(draft);
        }
        PageUpdate::StatusOnly
    }

    /// Cancel search: discard input and restore the original scroll position.
    pub fn cancel_search_input(&mut self) -> PageUpdate {
        if let PagerMode::SearchInput(mode) = mem::take(&mut self.mode) {
            log::debug!("Cancel search");
            self.jump_to(mode.start_line_index)
        } else {
            PageUpdate::StatusOnly
        }
    }

    /// Update the search input and scroll to the first match.
    pub fn update_search_query(&mut self, edit: LineEdit) -> PageUpdate {
        let PagerMode::SearchInput(mode) = &mut self.mode else {
            return PageUpdate::StatusOnly;
        };

        mode.editor.edit(edit);
        let input = mode.editor.input();

        if input.is_empty() {
            mode.draft = None;
            return PageUpdate::Partial(None);
        }

        let re = Regex::new(&regex::escape(&input)).unwrap();
        let matched = search::search_document(
            &mut self.doc,
            &re,
            SearchFrom::Line(mode.start_line_index),
            mode.direction,
        );
        log::debug!("Search preview: query={input:?}, result={matched:?}");

        let current_line_index = matched.as_ref().map(|m| m.line_index());
        mode.draft = Some(SearchState {
            query: re,
            direction: mode.direction,
            current: matched,
        });

        if let Some(line_index) = current_line_index {
            self.jump_to(line_index)
        } else {
            // Refresh the page to clear search highlights.
            PageUpdate::Partial(None)
        }
    }

    /// Jump to next/previous match using the stored search state.
    /// Returns `Some` when a match was found and applied, `None` otherwise.
    pub fn jump_to_next_match(&mut self, reverse: bool) -> Option<PageUpdate> {
        let next = self.find_next_match_position(reverse);
        log::debug!("Jump to next match: {next:?}");
        if let Some(pos) = next {
            let update = self.reveal_match(&pos);
            if let Some(s) = self.search.as_mut() {
                s.current = Some(pos);
                return Some(update);
            }
        }

        None
    }

    /// Move the page minimally so that the given match becomes visible.
    /// - If the match's row is already in the page, only refresh highlights (no scroll).
    /// - If the match is above the page, bring its line to the page top.
    /// - If the match is below the page, bring its line to the page bottom.
    fn reveal_match(&mut self, pos: &MatchPosition) -> PageUpdate {
        let raw_offset = match self.doc.line(pos.line_index()) {
            Some(line) => line.match_raw_range(pos).start,
            None => return PageUpdate::Partial(None),
        };

        let visible = self.contiguous_rows();
        let top = &visible[0];
        let bottom = &visible[visible.len() - 1];
        let target = pos.line_index();

        // Compare the match's row against the page edges by (line_index, raw_offset).
        // The match's row is off-page when it sorts before the top row or after the bottom row.
        let above = target < top.line_index()
            || (target == top.line_index() && raw_offset < top.raw_range().start);
        let below = target > bottom.line_index()
            || (target == bottom.line_index() && raw_offset >= bottom.raw_range().end);

        if above {
            self.jump_to(target)
        } else if below {
            self.jump_to_bottom(target)
        } else {
            // The match's row is within the page: just move the highlight, no scroll.
            PageUpdate::Partial(None)
        }
    }

    /// Find the next match to jump to.
    /// Handles re-anchoring when the current match is no longer visible in viewport.
    fn find_next_match_position(&mut self, reverse: bool) -> Option<MatchPosition> {
        let current_in_page = self.current_match_pos_in_page();
        let Some(ref search) = self.search else {
            log::debug!("Jump to next match: no active search");
            return None;
        };
        let (search_from, direction) = match current_in_page {
            Some(current) => {
                log::debug!("search '{}': current match in page", search.query);
                let direction = if reverse {
                    search.direction.opposite()
                } else {
                    search.direction
                };
                (SearchFrom::NextOf(current), direction)
            }
            None => {
                // When the current match is not visible,
                // jump to the first match in the page regardless of direction.
                log::debug!("search '{}': find first match in page", search.query);
                let top_row = self.contiguous_rows()[0].clone();
                (SearchFrom::Row(top_row), SearchDirection::Forward)
            }
        };
        search::search_document(&mut self.doc, &search.query, search_from, direction)
    }

    /// Return the current match position only if it is visible on screen.
    fn current_match_pos_in_page(&mut self) -> Option<MatchPosition> {
        let pos = self.search.as_ref().and_then(|s| s.current.as_ref())?;
        let line = self.doc.line(pos.line_index())?;
        let raw_offset = line.match_raw_range(pos).start;
        let is_in_page = self
            .contiguous_rows()
            .iter()
            .any(|r| r.line_index() == pos.line_index() && r.raw_range().contains(&raw_offset));
        if is_in_page { Some(pos.clone()) } else { None }
    }
}

/// A struct to calculate a proper [`PageUpdate`] for a jump.
///
/// It remembers the viewport edges before a jump and, after the jump, checks whether the old
/// and new viewports still overlap. When they do, the jump can be rendered as a scroll.
struct JumpDistance {
    prev_top: Row,
    prev_bottom: Row,
}

impl JumpDistance {
    fn from(viewport: &Viewport) -> Self {
        // Remember the viewport edges before the jump so we can measure the overlap afterwards.
        let rows = viewport.rows();
        Self {
            prev_top: rows[0].clone(),
            prev_bottom: rows[rows.len() - 1].clone(),
        }
    }

    fn compute(self, viewport: &Viewport) -> PageUpdate {
        let rows = viewport.rows();
        if self.prev_top < rows[0] {
            // Downward jump. The old top has scrolled off, but if the old bottom row is still
            // visible the viewports overlap, so we can render this as a downward scroll.
            match viewport.row_index(self.prev_bottom.line_index(), self.prev_bottom.wrap_index()) {
                Some(pos) => {
                    PageUpdate::Partial(Scroll::new(Direction::Down, rows.len() - 1 - pos))
                }
                None => PageUpdate::Full,
            }
        } else {
            // Upward jump (or no move). If the old top row is still within the new viewport,
            // the viewports overlap, so we can render this as an upward scroll.
            match viewport.row_index(self.prev_top.line_index(), self.prev_top.wrap_index()) {
                Some(pos) => PageUpdate::Partial(Scroll::new(Direction::Up, pos)),
                None => PageUpdate::Full,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::options::{HeadingOptions, Options};
    use regex::Regex;

    fn doc_lines(n: usize) -> Document {
        let s = (0..n)
            .map(|i| format!("line{i}"))
            .collect::<Vec<_>>()
            .join("\n");
        Document::from_string(s)
    }

    fn stream_msg(start: usize, count: usize) -> crate::document::StreamMsg {
        crate::document::StreamMsg::Lines(
            (0..count)
                .map(|i| crate::line::Line::new(start + i, format!("line{}", start + i)))
                .collect(),
        )
    }

    fn heading_opts(pattern: &str, num_lines: usize) -> HeadingOptions {
        HeadingOptions {
            pattern: Regex::new(pattern).unwrap(),
            num_lines,
        }
    }

    fn line_indices(rows: &[Row]) -> Vec<usize> {
        rows.iter().map(|r| r.line_index()).collect()
    }

    fn type_query(pager: &mut Pager, query: &str) {
        for ch in query.chars() {
            pager.update_search_query(LineEdit::AddChar(ch));
        }
    }

    fn submit_query(pager: &mut Pager, direction: SearchDirection, query: &str) {
        pager.start_search_input(direction);
        type_query(pager, query);
        pager.submit_search();
    }

    fn current_match_line(pager: &mut Pager) -> Option<usize> {
        let (snap, _doc) = pager.snapshot();
        snap.search
            .and_then(|s| s.current.as_ref().map(|m| m.line_index()))
    }

    #[test]
    fn pump_input_fills_first_screen_incrementally() {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut doc = Document::from_channel(rx);
        // One line must be available before constructing the pager.
        tx.send(stream_msg(0, 1)).unwrap();
        doc.pump();
        // viewport height = screen_height - 1 = 4.
        let mut pager = Pager::new(doc, Options::default(), ScreenSize::new(20, 5));
        {
            let (snap, _) = pager.snapshot();
            assert_eq!(line_indices(snap.content), vec![0]);
        }

        // More lines arrive: the first screen should fill from the top.
        tx.send(stream_msg(1, 3)).unwrap();
        let update = pager.pump_input();
        assert!(matches!(update, Some(PageUpdate::Full)));
        {
            let (snap, _) = pager.snapshot();
            assert_eq!(line_indices(snap.content), vec![0, 1, 2, 3]);
        }

        // Once the viewport is full, appended tail lines do not trigger a redraw.
        tx.send(stream_msg(4, 5)).unwrap();
        assert!(pager.pump_input().is_none());
        let (snap, _) = pager.snapshot();
        assert_eq!(line_indices(snap.content), vec![0, 1, 2, 3]);
    }

    #[test]
    fn jump_to_end_pumps_pending_input_first() {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut doc = Document::from_channel(rx);
        tx.send(stream_msg(0, 1)).unwrap();
        doc.pump();
        // viewport height = 4.
        let mut pager = Pager::new(doc, Options::default(), ScreenSize::new(20, 5));

        // More lines arrive but are not pumped in yet.
        tx.send(stream_msg(1, 9)).unwrap();
        pager.jump_to_end();

        // jump_to_end should have pumped the pending lines and landed on the
        // currently known end (lines 0..=9, last 4 visible).
        let (snap, _) = pager.snapshot();
        assert_eq!(line_indices(snap.content), vec![6, 7, 8, 9]);
    }

    #[test]
    fn pump_input_returns_none_when_nothing_arrived() {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut doc = Document::from_channel(rx);
        tx.send(stream_msg(0, 1)).unwrap();
        doc.pump();
        let mut pager = Pager::new(doc, Options::default(), ScreenSize::new(20, 5));
        assert!(pager.pump_input().is_none());
    }

    #[test]
    fn snapshot_starts_at_top_of_doc() {
        let mut pager = Pager::new(doc_lines(10), Options::default(), ScreenSize::new(20, 5));
        let (snap, _doc) = pager.snapshot();
        assert!(snap.header.is_empty());
        assert!(snap.heading.is_empty());
        // viewport height = screen_height - 1 = 4.
        assert_eq!(line_indices(snap.content), vec![0, 1, 2, 3]);
    }

    #[test]
    fn snapshot_pins_global_header_above_content() {
        let opts = Options {
            header: 2,
            ..Default::default()
        };
        let mut pager = Pager::new(doc_lines(10), opts, ScreenSize::new(20, 6));
        let (snap, _doc) = pager.snapshot();
        assert_eq!(line_indices(snap.header), vec![0, 1]);
        assert_eq!(line_indices(snap.content), vec![2, 3, 4]);
    }

    fn scroll_num_rows(update: Option<PageUpdate>) -> usize {
        match update {
            Some(PageUpdate::Partial(Some(scroll))) => scroll.num_rows.get(),
            _ => 0,
        }
    }

    #[test]
    fn scroll_down_shifts_content_forward() {
        let mut pager = Pager::new(doc_lines(20), Options::default(), ScreenSize::new(20, 5));
        let update = pager.scroll(2);
        assert_eq!(scroll_num_rows(update), 2);
        let (snap, _doc) = pager.snapshot();
        assert_eq!(line_indices(snap.content), vec![2, 3, 4, 5]);
    }

    #[test]
    fn scroll_up_brings_back_upper_rows() {
        let mut pager = Pager::new(doc_lines(20), Options::default(), ScreenSize::new(20, 5));
        pager.scroll(3);
        assert_eq!(line_indices(pager.snapshot().0.content), vec![3, 4, 5, 6]);
        let update = pager.scroll(-1);
        assert_eq!(scroll_num_rows(update), 1);
        assert_eq!(line_indices(pager.snapshot().0.content), vec![2, 3, 4, 5]);
    }

    #[test]
    fn scroll_keeps_global_header_pinned() {
        let opts = Options {
            header: 2,
            ..Default::default()
        };
        let mut pager = Pager::new(doc_lines(20), opts, ScreenSize::new(20, 6));
        pager.scroll(3);
        let (snap, _doc) = pager.snapshot();
        assert_eq!(line_indices(snap.header), vec![0, 1]);
        assert_eq!(line_indices(snap.content), vec![5, 6, 7]);
    }

    #[test]
    fn jump_to_places_target_line_at_top_of_content() {
        let mut pager = Pager::new(doc_lines(20), Options::default(), ScreenSize::new(20, 5));
        pager.jump_to(10);
        let (snap, _doc) = pager.snapshot();
        assert_eq!(line_indices(snap.content), vec![10, 11, 12, 13]);
    }

    fn jump_scroll(update: PageUpdate) -> Option<(Direction, usize)> {
        match update {
            PageUpdate::Partial(Some(scroll)) => Some((scroll.direction, scroll.num_rows.get())),
            _ => None,
        }
    }

    #[test]
    fn jump_to_downward_renders_as_partial_scroll_when_overlapping() {
        // Downward jump whose target was off-screen but still overlaps near the doc end.
        let mut pager = Pager::new(doc_lines(8), Options::default(), ScreenSize::new(20, 5));
        pager.scroll(1);
        assert_eq!(line_indices(pager.snapshot().0.content), vec![1, 2, 3, 4]);

        let update = pager.jump_to(6);
        assert_eq!(line_indices(pager.snapshot().0.content), vec![4, 5, 6, 7]);
        // Old bottom (line 4) is still visible, so this is a 3-row downward scroll.
        assert_eq!(jump_scroll(update), Some((Direction::Down, 3)));
    }

    #[test]
    fn jump_to_upward_renders_as_partial_scroll_when_overlapping() {
        let mut pager = Pager::new(doc_lines(20), Options::default(), ScreenSize::new(20, 5));
        pager.jump_to(10);
        assert_eq!(
            line_indices(pager.snapshot().0.content),
            vec![10, 11, 12, 13]
        );

        let update = pager.jump_to(8);
        assert_eq!(line_indices(pager.snapshot().0.content), vec![8, 9, 10, 11]);
        // Old top (line 10) is still visible, so this is a 2-row upward scroll.
        assert_eq!(jump_scroll(update), Some((Direction::Up, 2)));
    }

    #[test]
    fn jump_to_renders_full_when_no_overlap() {
        let mut pager = Pager::new(doc_lines(20), Options::default(), ScreenSize::new(20, 5));
        let update = pager.jump_to(10);
        assert_eq!(
            line_indices(pager.snapshot().0.content),
            vec![10, 11, 12, 13]
        );
        assert!(matches!(update, PageUpdate::Full));
    }

    #[test]
    fn jump_to_end_places_last_line_at_bottom() {
        let mut pager = Pager::new(doc_lines(20), Options::default(), ScreenSize::new(20, 5));
        pager.jump_to_end();
        let (snap, _doc) = pager.snapshot();
        assert_eq!(line_indices(snap.content), vec![16, 17, 18, 19]);
    }

    #[test]
    fn jump_to_within_global_header_jumps_to_top() {
        let opts = Options {
            header: 2,
            ..Default::default()
        };
        let mut pager = Pager::new(doc_lines(20), opts, ScreenSize::new(20, 8));
        pager.scroll(5);
        pager.jump_to(1);
        let (snap, _doc) = pager.snapshot();
        assert_eq!(line_indices(snap.header), vec![0, 1]);
        assert_eq!(line_indices(snap.content), vec![2, 3, 4, 5, 6]);
    }

    #[test]
    fn resize_rebuilds_content_at_new_height() {
        let mut pager = Pager::new(doc_lines(10), Options::default(), ScreenSize::new(20, 5));
        pager.resize(20, 10);
        let (snap, _doc) = pager.snapshot();
        // New viewport height = 9.
        assert_eq!(line_indices(snap.content), (0..9).collect::<Vec<_>>());
    }

    #[test]
    fn heading_becomes_sticky_when_scrolled_past() {
        let content = "# A\nx\ny\nz\nw\nv\n";
        let opts = Options {
            heading: Some(heading_opts("^# ", 1)),
            ..Default::default()
        };
        let mut pager = Pager::new(
            Document::from_string(content.into()),
            opts,
            ScreenSize::new(20, 5),
        );
        pager.scroll(2);
        let (snap, _doc) = pager.snapshot();
        assert_eq!(line_indices(snap.heading), vec![0]);
        assert_eq!(line_indices(snap.content), vec![3, 4, 5]);
    }

    #[test]
    fn fits_within_returns_true_for_short_doc() {
        let mut pager = Pager::new(doc_lines(5), Options::default(), ScreenSize::new(20, 10));
        assert!(pager.fits_within(5));
    }

    #[test]
    fn fits_within_returns_false_when_doc_exceeds_height() {
        let mut pager = Pager::new(doc_lines(6), Options::default(), ScreenSize::new(20, 10));
        assert!(!pager.fits_within(5));
    }

    #[test]
    fn content_height_excludes_header_rows() {
        let opts = Options {
            header: 1,
            ..Default::default()
        };
        let pager = Pager::new(doc_lines(10), opts, ScreenSize::new(20, 6));
        // viewport height = 5, header = 1 -> content = 4.
        assert_eq!(pager.content_height(), 4);
        assert_eq!(pager.total_header_height(), 1);
    }

    #[test]
    fn contiguous_rows_includes_global_header_when_adjacent() {
        let opts = Options {
            header: 2,
            ..Default::default()
        };
        let pager = Pager::new(doc_lines(10), opts, ScreenSize::new(20, 6));
        assert_eq!(line_indices(pager.contiguous_rows()), vec![0, 1, 2, 3, 4]);
    }

    #[test]
    fn contiguous_rows_excludes_global_header_when_far() {
        let opts = Options {
            header: 2,
            ..Default::default()
        };
        let mut pager = Pager::new(doc_lines(20), opts, ScreenSize::new(20, 6));
        pager.scroll(5);
        assert_eq!(line_indices(pager.contiguous_rows()), vec![7, 8, 9]);
    }

    #[test]
    fn contiguous_rows_includes_heading_when_adjacent() {
        let content = "# A\nline0\nline1\nline2\nline3\n";
        let opts = Options {
            heading: Some(heading_opts("^# ", 1)),
            ..Default::default()
        };
        let pager = Pager::new(
            Document::from_string(content.into()),
            opts,
            ScreenSize::new(20, 5),
        );
        assert_eq!(line_indices(pager.contiguous_rows()), vec![0, 1, 2, 3]);
    }

    #[test]
    fn snapshot_search_prefers_draft_over_committed_query() {
        let mut pager = Pager::new(doc_lines(20), Options::default(), ScreenSize::new(20, 5));
        pager.start_search_input(SearchDirection::Forward);
        type_query(&mut pager, "line5");
        pager.submit_search();

        // Start a new search: the draft input takes precedence over the committed query.
        pager.start_search_input(SearchDirection::Forward);
        type_query(&mut pager, "line8");
        let (snap, _doc) = pager.snapshot();
        let search = snap.search.expect("draft search should override committed");
        assert_eq!(search.query.as_str(), "line8");
    }

    #[test]
    fn contiguous_rows_excludes_heading_when_far() {
        let content = "# A\nline0\nline1\nline2\nline3\nline4\n";
        let opts = Options {
            heading: Some(heading_opts("^# ", 1)),
            ..Default::default()
        };
        let mut pager = Pager::new(
            Document::from_string(content.into()),
            opts,
            ScreenSize::new(20, 4),
        );
        pager.scroll(2);
        let rows = pager.contiguous_rows();
        // Heading (line 0) is no longer adjacent to content.
        assert_ne!(rows[0].line_index(), 0);
    }

    #[test]
    fn next_match_in_page_moves_highlight_without_scrolling() {
        let mut pager = Pager::new(doc_lines(20), Options::default(), ScreenSize::new(20, 5));
        submit_query(&mut pager, SearchDirection::Forward, "line");
        // "line" matches every line; the first match is line 0 and the page is [0,1,2,3].
        assert_eq!(line_indices(pager.snapshot().0.content), vec![0, 1, 2, 3]);
        assert_eq!(current_match_line(&mut pager), Some(0));

        let update = pager.jump_to_next_match(false);
        // The next match (line 1) is already in the page: no scroll, just refresh.
        assert!(matches!(update, Some(PageUpdate::Partial(None))));
        assert_eq!(line_indices(pager.snapshot().0.content), vec![0, 1, 2, 3]);
        assert_eq!(current_match_line(&mut pager), Some(1));
    }

    #[test]
    fn next_match_below_page_anchors_line_at_bottom() {
        let content = "a\nb\nc\nhit\ne\nf\ng\nhit\ni\nj\n";
        let mut pager = Pager::new(
            Document::from_string(content.into()),
            Options::default(),
            ScreenSize::new(20, 5),
        );
        // Submitting jumps to the first match (line 3) at the top: page [3,4,5,6].
        submit_query(&mut pager, SearchDirection::Forward, "hit");
        assert_eq!(line_indices(pager.snapshot().0.content), vec![3, 4, 5, 6]);
        assert_eq!(current_match_line(&mut pager), Some(3));

        pager.jump_to_next_match(false);
        // The next match (line 7) is below the page, so it is anchored at the bottom.
        let content = line_indices(pager.snapshot().0.content);
        assert_eq!(content, vec![4, 5, 6, 7]);
        assert_eq!(current_match_line(&mut pager), Some(7));
    }

    #[test]
    fn next_match_above_page_anchors_line_at_top() {
        let content = "a\nb\nc\nhit\ne\nf\ng\nhit\ni\nj\n";
        let mut pager = Pager::new(
            Document::from_string(content.into()),
            Options::default(),
            ScreenSize::new(20, 5),
        );
        submit_query(&mut pager, SearchDirection::Forward, "hit");
        pager.jump_to_next_match(false); // Move down to line 7: page [4,5,6,7].
        assert_eq!(line_indices(pager.snapshot().0.content), vec![4, 5, 6, 7]);

        pager.jump_to_next_match(true);
        // The previous match (line 3) is above the page, so it is anchored at the top.
        assert_eq!(line_indices(pager.snapshot().0.content), vec![3, 4, 5, 6]);
        assert_eq!(current_match_line(&mut pager), Some(3));
    }

    #[test]
    fn jump_to_bottom_shows_whole_wrapped_line() {
        // Width 5 wraps "01234hit" into ["01234", "hit"] (2 rows).
        let content = "a\nb\nc\nd\ne\nf\n01234hit\n";
        let mut pager = Pager::new(
            Document::from_string(content.into()),
            Options::default(),
            ScreenSize::new(5, 5),
        );

        pager.jump_to_bottom(6);
        let (snap, _doc) = pager.snapshot();
        // The whole wrapped line is shown, with its last wrap row at the bottom.
        let line6_wraps: Vec<usize> = snap
            .content
            .iter()
            .filter(|r| r.line_index() == 6)
            .map(|r| r.wrap_index())
            .collect();
        assert_eq!(line6_wraps, vec![0, 1]);
        let last = snap.content.last().unwrap();
        assert_eq!((last.line_index(), last.wrap_index()), (6, 1));
    }
}
