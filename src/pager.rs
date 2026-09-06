use std::mem;

use regex::Regex;

use crate::{
    document::Document,
    line::{MatchPosition, Row},
    line_editor::{LineEdit, LineEditor},
    options::Options,
    pager::layout::{Frame, Layout, RowPos},
    screen::ScreenSize,
    search::{self, SearchDirection, SearchFrom, SearchState},
};

mod layout;
mod rows;
mod status_line;

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
    draft: SearchDraft,
}

/// Live search state derived from the raw search input, before finalizing a query.
enum SearchDraft {
    /// Input is empty; there is no draft state to preview.
    Empty,
    /// Input compiled; the state matches the current input.
    Valid(SearchState),
    /// Input does not compile; preview stays frozen at the last valid state, if any.
    Invalid(Option<SearchState>),
}

impl SearchDraft {
    fn is_submittable(&self) -> bool {
        !matches!(self, SearchDraft::Invalid(_))
    }

    fn preview(&self) -> Option<&SearchState> {
        match self {
            SearchDraft::Valid(s) => Some(s),
            SearchDraft::Invalid(Some(s)) => Some(s),
            _ => None,
        }
    }

    fn into_preview(self) -> Option<SearchState> {
        match self {
            SearchDraft::Valid(s) => Some(s),
            SearchDraft::Invalid(s) => s,
            SearchDraft::Empty => None,
        }
    }
}

/// Centrally manages the pagination state.
/// [`Pager`] reads the rows that fit in the display area from [`Document`] and shows them
/// together with the status line. The whole display area is called the page, and the part
/// that shows rows of [`Document`] lines in particular is called the viewport.
/// Depending on the configuration, a global header and a heading may be pinned at
/// the top of the viewport.
///
/// The pinned rows are an overlay: they cover the first rows of the viewport rather than
/// pushing them down, which is what keeps scrolling uniform. Advancing the page by one row
/// always moves the visible content by exactly one row, whether or not a heading appeared
/// or disappeared in the same step.
///
/// The only page state [`Pager`] mutates is the anchor — the document row the viewport
/// starts at. Everything else, including which heading is pinned and how far it has been
/// pushed up, is derived by [`layout::compose`], so every operation reduces to picking an
/// anchor and recomposing.
/// [`Pager`] only holds the state but does not write anything to the screen itself.
pub struct Pager {
    doc: Document,
    mode: PagerMode,
    layout: Layout,
    frame: Frame,
    search: Option<SearchState>,
}

impl Pager {
    pub fn new(mut doc: Document, options: Options, screen_size: ScreenSize) -> Self {
        let size = ViewportSize::new(screen_size.width(), screen_size.height());
        let mut layout = Layout::new(options, size);
        let frame = layout::compose(&mut doc, &mut layout, (0, 0));
        Self {
            doc,
            mode: PagerMode::View,
            layout,
            frame,
            search: None,
        }
    }

    pub fn mode(&self) -> &PagerMode {
        &self.mode
    }

    pub fn doc(&self) -> &Document {
        &self.doc
    }

    pub fn doc_mut(&mut self) -> &mut Document {
        &mut self.doc
    }

    pub fn snapshot<'pager>(&'pager mut self) -> (PageSnapshot<'pager>, &'pager mut Document) {
        let search = match &self.mode {
            PagerMode::SearchInput(search) => search.draft.preview().or(self.search.as_ref()),
            _ => self.search.as_ref(),
        };
        let status_line = status_line::build(
            &self.mode,
            self.frame.rows(),
            self.layout.size().width(),
            &self.doc,
        );
        let snapshot = PageSnapshot {
            header: self.frame.header(),
            heading: self.frame.heading(),
            content: self.frame.content(),
            height: self.layout.size().height(),
            status_line,
            search,
        };
        (snapshot, &mut self.doc)
    }

    /// Rebuild the page for `anchor`. The composed frame may end up at a different anchor:
    /// [`layout::compose`] pulls it back when the page would otherwise be under-filled.
    fn compose_at(&mut self, anchor: RowPos) {
        self.frame = layout::compose(&mut self.doc, &mut self.layout, anchor);
    }

    /// Rebuild the page at the current anchor, for when the inputs to the layout changed
    /// rather than the position.
    fn recompose(&mut self) {
        let anchor = self.frame.anchor();
        self.compose_at(anchor);
    }

    /// Returns the height of the display area (the number of rows) excluding the header region.
    pub fn content_height(&self) -> usize {
        self.frame.content().len()
    }

    /// Whether the entire page fits within the specified `height`.
    pub fn fits_within(&mut self, height: usize) -> bool {
        let mut total_rows = 0;
        for i in 0..self.doc.line_count() {
            if let Some(line) = self.doc.line(i) {
                total_rows += line.row_count(self.layout.size().width());
                if total_rows > height {
                    return false;
                }
            }
        }
        true
    }

    /// Drain pending streamed input and reflect it in the page.
    ///
    /// While the first screen is still filling in (the viewport has not yet reached its
    /// full height), newly arrived lines extend the page from the top anchor.
    ///
    /// Once the viewport is full, appended tail lines stay below the fold, so the content
    /// does not change but the status line still does: its running total grows and the
    /// loading marker turns into the final percentage at EOF. Hence a `true` return does
    /// not imply the content moved.
    pub fn pump_input(&mut self) -> bool {
        let result = self.doc.pump();

        if result.grew && self.frame.rows().len() < self.layout.size().height() {
            self.recompose();
            return true;
        }

        result.grew || result.reached_eof
    }

    /// Whether more input may still arrive (the document is not yet complete).
    pub fn is_loading(&self) -> bool {
        !self.doc.is_complete()
    }

    /// Resize the page to fit the new dimensions.
    pub fn resize(&mut self, screen_width: usize, screen_height: usize) -> bool {
        let size = ViewportSize::new(screen_width, screen_height);
        self.layout.resize(size);
        self.recompose();
        true
    }

    /// Move the page so that the specified line comes to the top.
    /// - If the specified line is within the global header, jump to the start of the document.
    /// - If the specified line is within the heading that would be pinned, move so that it
    ///   comes right below the global header.
    /// - Otherwise, move so that the specified line comes right after the pinned rows.
    pub fn jump_to(&mut self, mut line_index: usize) -> bool {
        if self.layout.is_header_line(line_index) {
            line_index = 0;
        }

        let placement = layout::heading_placement(
            &mut self.doc,
            &mut self.layout,
            self.frame.header(),
            line_index,
        );
        let heading_height = match &placement {
            // The target is one of the heading lines: show it as the pinned heading itself.
            Some(p) if p.lines.contains(&line_index) => 0,
            Some(p) => p.height,
            None => 0,
        };

        let rows_above = self.frame.header().len() + heading_height;
        let anchor =
            layout::anchor_backward(&mut self.doc, &self.layout, (line_index, 0), rows_above);
        self.compose_at(anchor);
        true
    }

    /// Jump to the end of the document so that the last line is at the bottom.
    /// For streamed input this jumps to the currently known end (non-blocking);
    /// lines still arriving become reachable as they are pumped in.
    pub fn jump_to_end(&mut self) -> bool {
        self.doc.pump();
        let anchor = layout::end_anchor(&mut self.doc, &self.layout);
        self.compose_at(anchor);
        true
    }

    /// Move the page so that the specified line is fully shown with its last row at the bottom.
    /// Unlike [`Self::jump_to`], which anchors the line at the top, this anchors the whole line
    /// at the bottom so that wherever a match sits within the line it stays visible.
    fn jump_to_bottom(&mut self, line_index: usize) -> bool {
        let width = self.layout.size().width();
        let row_count = self
            .doc
            .line(line_index)
            .map(|l| l.row_count(width))
            .unwrap_or(1);
        let rows_above = self.layout.size().height().saturating_sub(row_count);
        let anchor =
            layout::anchor_backward(&mut self.doc, &self.layout, (line_index, 0), rows_above);
        self.compose_at(anchor);
        true
    }

    /// Scroll by the given number of rows (positive = down, negative = up).
    /// Returns whether the page actually moved.
    pub fn scroll(&mut self, num_rows: i32) -> bool {
        let before = self.frame.anchor();
        let anchor = match num_rows {
            0 => return false,
            n if n < 0 => {
                layout::anchor_backward(&mut self.doc, &self.layout, before, (-n) as usize)
            }
            n => layout::anchor_forward(&mut self.doc, &self.layout, before, n as usize),
        };

        self.compose_at(anchor);
        self.frame.anchor() != before
    }

    pub fn has_search_input(&self) -> bool {
        match &self.mode {
            PagerMode::SearchInput(mode) => !mode.editor.is_empty(),
            _ => false,
        }
    }

    pub fn start_search_input(&mut self, direction: SearchDirection) -> bool {
        // Search from the top of the contiguous rows, which include the sticky rows while
        // they sit directly above the content. A match in those rows must be reachable
        // during the preview too; otherwise the cursor would start in the content and only
        // move up into the sticky rows via n/N after the query is submitted.
        let start_line_index = self.frame.contiguous_rows()[0].line_index();
        let editor = LineEditor::new();
        self.mode = PagerMode::SearchInput(SearchInputMode {
            direction,
            editor,
            start_line_index,
            draft: SearchDraft::Empty,
        });
        true
    }

    /// Commit the current search input.
    /// Does nothing and keeps the search input mode active if the current raw input
    /// is not a valid regex, so the user can keep editing it.
    pub fn submit_search(&mut self) -> bool {
        let PagerMode::SearchInput(mode) = &mut self.mode else {
            return false;
        };
        if !mode.draft.is_submittable() {
            return false;
        }
        if let SearchDraft::Valid(draft) = mem::replace(&mut mode.draft, SearchDraft::Empty) {
            log::debug!("Submit search: query={:?}", draft.query.as_str());
            self.search = Some(draft);
        }
        self.mode = PagerMode::View;
        true
    }

    /// Cancel search: discard input and restore the original scroll position.
    pub fn cancel_search_input(&mut self) -> bool {
        if let PagerMode::SearchInput(mode) = mem::take(&mut self.mode) {
            log::debug!("Cancel search");
            self.jump_to(mode.start_line_index);
        }
        true
    }

    /// Update the search input and scroll to the first match.
    pub fn update_search_query(&mut self, edit: LineEdit) -> bool {
        let PagerMode::SearchInput(mode) = &mut self.mode else {
            return false;
        };

        let changes_text = edit.changes_text();
        mode.editor.edit(edit);
        if !changes_text {
            return true;
        }
        let input = mode.editor.input();

        if input.is_empty() {
            mode.draft = SearchDraft::Empty;
            return true;
        }

        // While the input is mid-edit (e.g. right after typing `(` or `[`), it is often
        // a syntactically invalid regex. Freeze the preview at its last valid state
        // instead of clearing it, so the search results don't flicker away.
        let Ok(re) = Regex::new(&input) else {
            let frozen = mem::replace(&mut mode.draft, SearchDraft::Empty).into_preview();
            mode.draft = SearchDraft::Invalid(frozen);
            return true;
        };

        let matched = search::search_document(
            &mut self.doc,
            &re,
            SearchFrom::Line(mode.start_line_index),
            mode.direction,
        );
        log::debug!("Search preview: query={input:?}, result={matched:?}");

        let current_line_index = matched.as_ref().map(|m| m.line_index());
        mode.draft = SearchDraft::Valid(SearchState {
            query: re,
            direction: mode.direction,
            current: matched,
        });

        if let Some(line_index) = current_line_index {
            self.jump_to(line_index);
        }
        true
    }

    /// Jump to next/previous match using the stored search state.
    /// Returns whether a match was found and applied.
    pub fn jump_to_next_match(&mut self, reverse: bool) -> bool {
        let next = self.find_next_match_position(reverse);
        log::debug!("Jump to next match: {next:?}");
        let Some(pos) = next else {
            return false;
        };
        self.reveal_match(&pos);
        match self.search.as_mut() {
            Some(s) => {
                s.current = Some(pos);
                true
            }
            None => false,
        }
    }

    /// Move the page minimally so that the given match becomes visible.
    /// - If the match's row is already in the page, only refresh highlights (no scroll).
    /// - If the match is above the page, bring its line to the page top.
    /// - If the match is below the page, bring its line to the page bottom.
    fn reveal_match(&mut self, pos: &MatchPosition) {
        let raw_offset = match self.doc.line(pos.line_index()) {
            Some(line) => line.match_raw_range(pos).start,
            None => return,
        };

        let visible = self.frame.contiguous_rows();
        let top = &visible[0];
        let bottom = &visible[visible.len() - 1];
        let target = pos.line_index();

        // The match is off-page when it sorts before the top row or after the bottom row,
        // ordering by (line_index, raw_offset) so a wrapped line is compared per row.
        let above = target < top.line_index()
            || (target == top.line_index() && raw_offset < top.raw_range().start);
        let below = target > bottom.line_index()
            || (target == bottom.line_index() && raw_offset >= bottom.raw_range().end);

        if above {
            self.jump_to(target);
        } else if below {
            self.jump_to_bottom(target);
        }
        // Otherwise the match's row is already in the page: only the highlight moves.
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
                let top_row = self.frame.contiguous_rows()[0].clone();
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
            .frame
            .contiguous_rows()
            .iter()
            .any(|r| r.line_index() == pos.line_index() && r.raw_range().contains(&raw_offset));
        if is_in_page { Some(pos.clone()) } else { None }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::StreamMsg;
    use crate::line::Line;
    use crate::options::{HeadingOptions, Options};
    use crate::pager::status_line::{STATUS_REVERSE_OFF, STATUS_REVERSE_ON};
    use regex::Regex;
    use std::sync::mpsc;

    fn doc_lines(n: usize) -> Document {
        let s = (0..n)
            .map(|i| format!("line{i}"))
            .collect::<Vec<_>>()
            .join("\n");
        Document::from_string(s)
    }

    fn send_lines(tx: &mpsc::Sender<StreamMsg>, start: usize, count: usize) {
        for i in 0..count {
            let line = Line::new(start + i, format!("line{}", start + i));
            tx.send(StreamMsg::Line(line)).unwrap();
        }
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

    /// View-mode status content with the reverse-video wrapper stripped, so
    /// assertions can focus on the text. Also asserts the wrapper is present.
    fn view_status(pager: &mut Pager) -> String {
        let s = pager.snapshot().0.status_line;
        s.strip_prefix(STATUS_REVERSE_ON)
            .and_then(|s| s.strip_suffix(STATUS_REVERSE_OFF))
            .expect("view-mode status should be reverse-wrapped")
            .to_string()
    }

    #[test]
    fn status_line_shows_position_for_string_source() {
        // 5 lines, viewport height 4: content covers lines 1-4 of 5 -> 80%.
        let mut pager = Pager::new(doc_lines(5), Options::default(), ScreenSize::new(20, 5));
        assert_eq!(view_status(&mut pager), "lines 1-4/5 80%");

        pager.scroll(1);
        assert_eq!(view_status(&mut pager), "lines 2-5/5 100%");
    }

    #[test]
    fn status_line_marks_loading_then_settles_on_eof() {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut doc = Document::from_channel(rx);
        send_lines(&tx, 0, 1);
        doc.pump();
        let mut pager = Pager::new(doc, Options::default(), ScreenSize::new(20, 5));

        // Still streaming: total is a growing lower bound, no percentage.
        assert_eq!(view_status(&mut pager), "lines 1-1/1+");

        // Once the rest arrives and EOF is reached, the total is final.
        send_lines(&tx, 1, 9);
        tx.send(crate::document::StreamMsg::Eof).unwrap();
        pager.pump_input();
        assert_eq!(view_status(&mut pager), "lines 1-4/10 40%");
    }

    #[test]
    fn status_line_flags_read_error_instead_of_percentage() {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut doc = Document::from_channel(rx);
        send_lines(&tx, 0, 1);
        doc.pump();
        let mut pager = Pager::new(doc, Options::default(), ScreenSize::new(40, 5));

        // The reader fails after one line: the status flags the truncation
        // instead of settling on a misleading final percentage.
        tx.send(crate::document::StreamMsg::Error(std::io::Error::other(
            "boom",
        )))
        .unwrap();
        pager.pump_input();
        assert_eq!(view_status(&mut pager), "lines 1-1/1 [read error]");
    }

    #[test]
    fn status_line_prefixes_file_name() {
        let dir = std::path::Path::new(".local/test");
        std::fs::create_dir_all(dir).unwrap();
        let path = dir.join("status_line_name.txt");
        std::fs::write(&path, "a\nb\nc\nd\ne\n").unwrap();

        let doc = Document::from_file(&path).unwrap();
        let mut pager = Pager::new(doc, Options::default(), ScreenSize::new(80, 5));
        let name = path.display();
        assert_eq!(view_status(&mut pager), format!("{name} lines 1-4/5 80%"));

        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn status_line_is_reverse_in_view_but_plain_in_search_input() {
        let mut pager = Pager::new(doc_lines(5), Options::default(), ScreenSize::new(20, 5));
        // View mode: wrapped in reverse video.
        let view = pager.snapshot().0.status_line;
        assert!(view.starts_with(STATUS_REVERSE_ON) && view.ends_with(STATUS_REVERSE_OFF));

        // Search input: plain, no reverse-video wrapper.
        pager.start_search_input(SearchDirection::Forward);
        type_query(&mut pager, "line");
        let input = pager.snapshot().0.status_line;
        assert!(!input.contains(STATUS_REVERSE_ON));
        assert!(input.starts_with('/'));
    }

    #[test]
    fn pump_input_fills_first_screen_incrementally() {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut doc = Document::from_channel(rx);
        // One line must be available before constructing the pager.
        send_lines(&tx, 0, 1);
        doc.pump();
        // viewport height = screen_height - 1 = 4.
        let mut pager = Pager::new(doc, Options::default(), ScreenSize::new(20, 5));
        {
            let (snap, _) = pager.snapshot();
            assert_eq!(line_indices(snap.content), vec![0]);
        }

        // More lines arrive: the first screen should fill from the top.
        send_lines(&tx, 1, 3);
        let update = pager.pump_input();
        assert!(update);
        {
            let (snap, _) = pager.snapshot();
            assert_eq!(line_indices(snap.content), vec![0, 1, 2, 3]);
        }

        // Once the viewport is full, appended tail lines stay below the fold, so
        // the content is unchanged, but the status line still needs refreshing for
        // the growing total.
        send_lines(&tx, 4, 5);
        assert!(pager.pump_input());
        let (snap, _) = pager.snapshot();
        assert_eq!(line_indices(snap.content), vec![0, 1, 2, 3]);

        // A pump that drains nothing changes nothing.
        assert!(!pager.pump_input());
    }

    #[test]
    fn pump_input_refreshes_status_on_eof_after_first_screen() {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut doc = Document::from_channel(rx);
        send_lines(&tx, 0, 5);
        doc.pump();
        // viewport height = 4, so the first screen is already full.
        let mut pager = Pager::new(doc, Options::default(), ScreenSize::new(20, 5));
        assert!(pager.is_loading());

        // EOF with no new line still refreshes the status line to drop the marker.
        tx.send(crate::document::StreamMsg::Eof).unwrap();
        assert!(pager.pump_input());
        assert!(!pager.is_loading());
    }

    #[test]
    fn jump_to_end_pumps_pending_input_first() {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut doc = Document::from_channel(rx);
        send_lines(&tx, 0, 1);
        doc.pump();
        // viewport height = 4.
        let mut pager = Pager::new(doc, Options::default(), ScreenSize::new(20, 5));

        // More lines arrive but are not pumped in yet.
        send_lines(&tx, 1, 9);
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
        send_lines(&tx, 0, 1);
        doc.pump();
        let mut pager = Pager::new(doc, Options::default(), ScreenSize::new(20, 5));
        assert!(!pager.pump_input());
    }

    /// A document not yet as long as `--header-lines` gets a shorter header rather than a
    /// padded one: it shows the lines that exist, and grows to its configured size as the
    /// rest arrives.
    #[test]
    fn header_shows_only_the_lines_that_exist_until_the_document_catches_up() {
        let (tx, rx) = mpsc::channel();
        let mut doc = Document::from_channel(rx);
        // Only 2 lines have arrived, but --header-lines is configured to 3.
        send_lines(&tx, 0, 2);
        doc.pump();
        // viewport height = 9, far more than a 3-line header needs.
        let opts = Options {
            header: 3,
            ..Default::default()
        };
        let mut pager = Pager::new(doc, opts, ScreenSize::new(20, 10));

        // The header shows only the 2 lines that exist, not padded to 3.
        let (snap, _) = pager.snapshot();
        assert_eq!(line_indices(snap.header), vec![0, 1]);

        // Once the rest of the document arrives, the header reaches its configured size.
        send_lines(&tx, 2, 3);
        pager.pump_input();
        let (snap, _) = pager.snapshot();
        assert_eq!(line_indices(snap.header), vec![0, 1, 2]);
    }

    /// `--header-lines` claims its lines up front, whether or not the document is that long
    /// yet. A claimed line never becomes the sticky heading, not even once it finally arrives,
    /// while the first line past the claimed range does.
    #[test]
    fn heading_stays_outside_the_configured_header_while_the_document_is_still_shorter() {
        let (tx, rx) = mpsc::channel();
        let mut doc = Document::from_channel(rx);
        // Only line 0 has arrived; line 1 ("# B"), inside the header, hasn't yet.
        tx.send(StreamMsg::Line(Line::new(0, "# A".into())))
            .unwrap();
        doc.pump();

        let opts = Options {
            header: 3,
            heading: Some(heading_opts("^# ", 1)),
            ..Default::default()
        };
        let mut pager = Pager::new(doc, opts, ScreenSize::new(20, 10));

        // Only line 0 exists, so the header occupies a single row for the 3 lines it is
        // configured to cover, and nothing is a heading while the rest is still missing.
        let (snap, _) = pager.snapshot();
        assert_eq!(line_indices(snap.header), vec![0]);
        assert!(line_indices(snap.heading).is_empty());

        // The rest of the document arrives, including "# B" (line 1, still inside the
        // header) and "# D" (line 3, the first line outside the header).
        for (i, text) in [(1, "# B"), (2, "# C"), (3, "# D"), (4, "tail")] {
            tx.send(StreamMsg::Line(Line::new(i, text.into()))).unwrap();
        }
        tx.send(StreamMsg::Eof).unwrap();
        pager.pump_input();

        // "# B" (line 1) is inside the header: jumping to it redirects to the top of the
        // document instead of making it a sticky heading. What gets pinned there is
        // "# D" (line 3), the first line outside the header, never a header line.
        pager.jump_to(1);
        assert_eq!(line_indices(pager.snapshot().0.heading), vec![3]);

        // "# D" (line 3) is the first line outside the header: it becomes the heading.
        pager.jump_to(3);
        assert_eq!(line_indices(pager.snapshot().0.heading), vec![3]);
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

    #[test]
    fn scroll_down_shifts_content_forward() {
        let mut pager = Pager::new(doc_lines(20), Options::default(), ScreenSize::new(20, 5));
        assert!(pager.scroll(2));
        let (snap, _doc) = pager.snapshot();
        assert_eq!(line_indices(snap.content), vec![2, 3, 4, 5]);
    }

    #[test]
    fn scroll_up_brings_back_upper_rows() {
        let mut pager = Pager::new(doc_lines(20), Options::default(), ScreenSize::new(20, 5));
        pager.scroll(3);
        assert_eq!(line_indices(pager.snapshot().0.content), vec![3, 4, 5, 6]);
        assert!(pager.scroll(-1));
        assert_eq!(line_indices(pager.snapshot().0.content), vec![2, 3, 4, 5]);
    }

    #[test]
    fn scroll_reports_no_change_at_the_document_edges() {
        let mut pager = Pager::new(doc_lines(6), Options::default(), ScreenSize::new(20, 5));
        // Already at the top: there is nothing above to scroll to.
        assert!(!pager.scroll(-1));

        pager.jump_to_end();
        assert_eq!(line_indices(pager.snapshot().0.content), vec![2, 3, 4, 5]);
        assert!(!pager.scroll(1));
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

    #[test]
    fn jump_to_near_the_document_end_keeps_the_page_full() {
        // The target is too close to the end to sit at the top, so the page fills from above.
        let mut pager = Pager::new(doc_lines(8), Options::default(), ScreenSize::new(20, 5));
        pager.jump_to(6);
        assert_eq!(line_indices(pager.snapshot().0.content), vec![4, 5, 6, 7]);
    }

    #[test]
    fn jump_to_upward_places_target_line_at_top() {
        let mut pager = Pager::new(doc_lines(20), Options::default(), ScreenSize::new(20, 5));
        pager.jump_to(10);
        assert_eq!(
            line_indices(pager.snapshot().0.content),
            vec![10, 11, 12, 13]
        );

        pager.jump_to(8);
        assert_eq!(line_indices(pager.snapshot().0.content), vec![8, 9, 10, 11]);
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
    }

    #[test]
    fn contiguous_rows_includes_global_header_when_adjacent() {
        let opts = Options {
            header: 2,
            ..Default::default()
        };
        let pager = Pager::new(doc_lines(10), opts, ScreenSize::new(20, 6));
        assert_eq!(
            line_indices(&pager.frame.contiguous_rows()),
            vec![0, 1, 2, 3, 4]
        );
    }

    #[test]
    fn contiguous_rows_excludes_global_header_when_far() {
        let opts = Options {
            header: 2,
            ..Default::default()
        };
        let mut pager = Pager::new(doc_lines(20), opts, ScreenSize::new(20, 6));
        pager.scroll(5);
        assert_eq!(line_indices(&pager.frame.contiguous_rows()), vec![7, 8, 9]);
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
        assert_eq!(
            line_indices(&pager.frame.contiguous_rows()),
            vec![0, 1, 2, 3]
        );
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
    fn cursor_move_in_search_input_does_not_rerun_search() {
        let mut pager = Pager::new(doc_lines(20), Options::default(), ScreenSize::new(20, 5));
        pager.start_search_input(SearchDirection::Forward);
        type_query(&mut pager, "line5");
        assert_eq!(current_match_line(&mut pager), Some(5));

        pager.update_search_query(LineEdit::MoveCursorLeft);
        assert_eq!(current_match_line(&mut pager), Some(5));

        pager.update_search_query(LineEdit::MoveCursorRight);
        assert_eq!(current_match_line(&mut pager), Some(5));
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
        let rows = pager.frame.contiguous_rows();
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

        // The next match (line 1) is already in the page: the page does not move.
        assert!(pager.jump_to_next_match(false));
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
