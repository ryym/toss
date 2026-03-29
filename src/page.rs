use crate::document::Document;
use crate::header::Header;
use crate::options::Options;
use crate::status_line::StatusLine;
use crate::viewport::{ScreenRow, ScrollPlan, Viewport};

/// Resolved header layout for the current viewport position.
pub struct HeaderLayout {
    pub(crate) rows: Vec<ScreenRow>,
    pub(crate) fixed_height: usize,
}

impl HeaderLayout {
    /// All header rows (fixed + section overlay).
    #[inline]
    pub fn rows(&self) -> &[ScreenRow] {
        &self.rows
    }

    /// Total height of the header (fixed + section overlay).
    #[inline]
    pub fn height(&self) -> usize {
        self.rows.len()
    }

    /// Number of screen rows occupied by the fixed header.
    #[inline]
    pub fn fixed_height(&self) -> usize {
        self.fixed_height
    }

    /// Number of screen rows overlaied on the viewport.
    #[inline]
    pub fn overlay_height(&self) -> usize {
        self.rows.len() - self.fixed_height
    }
}

/// Bundles the document, header, viewport, and status line — everything the
/// rendering functions need to draw a frame (except search highlight).
pub struct Page {
    pub doc: Document,
    header: Header,
    pub viewport: Viewport,
    pub status: StatusLine,
}

impl Page {
    /// Create a new page with the initial viewport sized for the given terminal dimensions.
    pub fn new(mut doc: Document, options: &Options, width: usize, height: usize) -> Self {
        let header = Header::new(options.header, options.section.as_ref());
        let header_height = header.resolve_fixed_height(&mut doc, width);
        let content_height = height.saturating_sub(1).saturating_sub(header_height);
        let viewport = Viewport::new(&mut doc, width, content_height, header.fixed_line_len());
        let status = StatusLine::new();
        Self {
            doc,
            header,
            viewport,
            status,
        }
    }

    /// Whether the entire document content fits on the terminal when printed
    /// to stdout (no pager UI). `shell_lines` reserves space for the shell
    /// prompt that will appear after the output (like LESS_SHELL_LINES).
    pub fn content_fits_on_screen(&mut self, screen_height: usize, shell_lines: usize) -> bool {
        let width = self.viewport.width();
        let available = screen_height.saturating_sub(shell_lines);
        let mut total_rows = 0;
        for i in 0..self.doc.line_count() {
            if let Some(line) = self.doc.line(i) {
                total_rows += line.row_count(width);
                if total_rows > available {
                    return false;
                }
            }
        }
        total_rows <= available
    }

    /// Resize the viewport to fit the new terminal dimensions, accounting for
    /// the header and status line.
    pub fn resize(&mut self, width: usize, height: usize) {
        let header = self.resolve_header_within(width, true);
        let content_height = height
            .saturating_sub(1)
            .saturating_sub(header.fixed_height());
        self.viewport.resize(&mut self.doc, width, content_height);
    }

    /// Scroll by the given number of rows (positive = down, negative = up).
    /// Returns a ScrollPlan if scrolling occurred, None otherwise.
    pub fn plan_scroll(&mut self, rows: isize) -> Option<ScrollPlan> {
        let old_top = self.viewport.top_line_index();
        let plan = if rows > 0 {
            self.viewport.scroll_down(rows as usize, &mut self.doc)
        } else {
            self.viewport.scroll_up((-rows) as usize, &mut self.doc)
        };
        if plan.is_some() {
            let new_top = self.viewport.top_line_index();
            self.header
                .update_section_on_scroll(&mut self.doc, old_top, new_top, rows > 0);
        }
        plan
    }

    /// Jump to a line, ensuring it is visible below any sticky section header.
    /// Iteratively scrolls up until the target line clears the overlay, because
    /// with multi-line section headers the push-up mechanism can increase the
    /// overlay as the viewport moves up.
    pub fn jump_to_visible(&mut self, line: usize) -> bool {
        let mut changed = self.viewport.jump_to(&mut self.doc, line);
        loop {
            self.resolve_header_synced();
            let overlay = self.viewport.overlay_len();
            let target_row = self
                .viewport
                .rows()
                .iter()
                .position(|r| r.line_index == line && r.wrap_index == 0);
            match target_row {
                Some(row) if row >= overlay => break,
                _ => {
                    if self.viewport.scroll_up(1, &mut self.doc).is_none() {
                        break;
                    }
                    changed = true;
                }
            }
        }
        changed
    }

    /// Resolve the header rows for the current viewport width.
    /// Uses `sync_section=false` (assumes cache is already up to date from scroll).
    /// Updates the viewport overlay.
    pub fn resolve_header(&mut self) -> HeaderLayout {
        self.resolve_header_within(self.viewport.width(), false)
    }

    /// Resolve the header rows, synchronizing the section index cache.
    /// Used on full redraws where the cache may be stale.
    /// Updates the viewport overlay.
    pub fn resolve_header_synced(&mut self) -> HeaderLayout {
        self.resolve_header_within(self.viewport.width(), true)
    }

    fn resolve_header_within(&mut self, width: usize, sync_section: bool) -> HeaderLayout {
        let header = self.header.resolve(
            &mut self.doc,
            width,
            self.viewport.top_line_index(),
            self.viewport.top_wrap_index(),
            sync_section,
        );
        self.viewport.set_overlay_len(header.overlay_height());
        header
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn make_page(content: &str, options: &Options, width: usize, height: usize) -> Page {
        let doc = Document::from_string(content.to_string());
        Page::new(doc, options, width, height)
    }

    #[test]
    fn fits_when_fewer_lines_than_screen() {
        // Screen height 5, shell_lines 1: available = 4. 3 lines <= 4.
        let mut page = make_page("line 1\nline 2\nline 3", &Options::default(), 80, 5);
        assert_eq!(page.content_fits_on_screen(5, 1), true);
    }

    #[test]
    fn fits_when_lines_equal_available() {
        // Screen height 5, shell_lines 1: available = 4. 4 lines <= 4.
        let mut page = make_page("line 1\nline 2\nline 3\nline 4", &Options::default(), 80, 5);
        assert_eq!(page.content_fits_on_screen(5, 1), true);
    }

    #[test]
    fn does_not_fit_when_lines_exceed_screen() {
        // Screen height 5, shell_lines 1: available = 4. 5 lines > 4.
        let mut page = make_page(
            "line 1\nline 2\nline 3\nline 4\nline 5",
            &Options::default(),
            80,
            5,
        );
        assert_eq!(page.content_fits_on_screen(5, 1), false);
    }

    #[test]
    fn shell_lines_reduces_available_space() {
        // Screen height 5, shell_lines 2: available = 3. 4 lines > 3.
        let mut page = make_page("line 1\nline 2\nline 3\nline 4", &Options::default(), 80, 5);
        assert_eq!(page.content_fits_on_screen(5, 2), false);
        // Same content with shell_lines 1: available = 4. 4 lines <= 4.
        assert_eq!(page.content_fits_on_screen(5, 1), true);
    }

    #[test]
    fn wrapping_increases_row_count() {
        // "abcdefghij" (10 chars) wraps to 2 rows at width 5.
        // Screen height 5, shell_lines 1: available = 4. 2+1 = 3 rows <= 4.
        let mut page = make_page("abcdefghij\nshort", &Options::default(), 5, 5);
        assert_eq!(page.content_fits_on_screen(5, 1), true);
        // Screen height 4, shell_lines 1: available = 3. 3 rows <= 3.
        assert_eq!(page.content_fits_on_screen(4, 1), true);
        // Screen height 3, shell_lines 1: available = 2. 3 rows > 2.
        assert_eq!(page.content_fits_on_screen(3, 1), false);
    }

    #[test]
    fn empty_document_fits() {
        let mut page = make_page("", &Options::default(), 80, 5);
        assert_eq!(page.content_fits_on_screen(5, 1), true);
    }
}
