use crate::document::Document;
use crate::header::Header;
use crate::options::Options;
use crate::status_line::StatusLine;
use crate::viewport::{ScreenRow, ScrollPlan, Viewport};

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
        let viewport = Viewport::new(&mut doc, width, content_height, header.min_top_line());
        let status = StatusLine::new();
        Self {
            doc,
            header,
            viewport,
            status,
        }
    }

    /// Whether the entire document content fits within the viewport.
    pub fn content_fits_on_screen(&self) -> bool {
        self.viewport.rows().len() < self.viewport.height()
    }

    /// Resize the viewport to fit the new terminal dimensions, accounting for
    /// the header and status line.
    pub fn resize(&mut self, width: usize, height: usize) {
        let header_height = self
            .header
            .resolve(
                &mut self.doc,
                width,
                self.viewport.top_line_index(),
                self.viewport.top_wrap_index(),
                true,
            )
            .len();
        let overlay = self.header.section_overlay();
        let content_height = height
            .saturating_sub(1)
            .saturating_sub(header_height - overlay);
        self.viewport.resize(&mut self.doc, width, content_height);
    }

    /// Scroll by the given number of rows (positive = down, negative = up).
    /// Returns `None` if no scrolling occurred (e.g. already at boundary).
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

    /// Returns the current sticky section start line, if any.
    pub fn current_section(&self) -> Option<usize> {
        self.header.current_section()
    }

    /// Number of viewport rows overlaid by the section header.
    pub fn section_overlay(&self) -> usize {
        self.header.section_overlay()
    }

    /// Resolve the header rows for the current viewport width.
    /// Uses `sync_section=false` (assumes cache is already up to date from scroll).
    pub fn resolve_header(&mut self) -> Vec<ScreenRow> {
        let width = self.viewport.width();
        let top = self.viewport.top_line_index();
        let top_wrap = self.viewport.top_wrap_index();
        self.header
            .resolve(&mut self.doc, width, top, top_wrap, false)
    }

    /// Resolve the header rows, synchronizing the section index cache.
    /// Used on full redraws where the cache may be stale.
    pub fn resolve_header_synced(&mut self) -> Vec<ScreenRow> {
        let width = self.viewport.width();
        let top = self.viewport.top_line_index();
        let top_wrap = self.viewport.top_wrap_index();
        self.header
            .resolve(&mut self.doc, width, top, top_wrap, true)
    }

    /// Synchronize the section cache and adjust viewport if header height changed.
    /// Call this before a full redraw to ensure the viewport is correctly sized.
    /// May need multiple iterations if resizing changes the viewport top, which
    /// in turn changes which section is sticky.
    pub fn sync_section_for_redraw(&mut self, screen_height: usize) {
        loop {
            let header_height = self.resolve_header_synced().len();
            let overlay = self.header.section_overlay();
            let content_height = screen_height
                .saturating_sub(1)
                .saturating_sub(header_height - overlay);
            if content_height == self.viewport.height() {
                break;
            }
            self.viewport
                .resize(&mut self.doc, self.viewport.width(), content_height);
        }
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
        // Screen height 5: 1 status line + 4 content rows. 3 lines < 4 rows.
        let page = make_page("line 1\nline 2\nline 3", &Options::default(), 80, 5);
        assert_eq!(page.content_fits_on_screen(), true);
    }

    #[test]
    fn does_not_fit_when_lines_fill_screen() {
        // Screen height 5: 1 status line + 4 content rows. 4 lines == 4 rows.
        let page = make_page("line 1\nline 2\nline 3\nline 4", &Options::default(), 80, 5);
        assert_eq!(page.content_fits_on_screen(), false);
    }

    #[test]
    fn does_not_fit_when_lines_exceed_screen() {
        // Screen height 5: 1 status line + 4 content rows. 5 lines > 4 rows.
        let page = make_page(
            "line 1\nline 2\nline 3\nline 4\nline 5",
            &Options::default(),
            80,
            5,
        );
        assert_eq!(page.content_fits_on_screen(), false);
    }

    #[test]
    fn wrapping_increases_row_count() {
        // "abcdefghij" (10 chars) wraps to 2 rows at width 5.
        // Screen height 4: 1 status + 3 content rows. 2 lines => 3 rows (wrapping).
        let page = make_page("abcdefghij\nshort", &Options::default(), 5, 4);
        assert_eq!(page.content_fits_on_screen(), false);
    }

    #[test]
    fn fits_with_header() {
        let options = Options {
            header: 1,
            ..Default::default()
        };
        // Screen height 5: 1 status + 1 header + 3 content rows.
        // Content below header: 2 lines < 3 rows.
        let page = make_page("header\nline 1\nline 2", &options, 80, 5);
        assert_eq!(page.content_fits_on_screen(), true);
    }

    #[test]
    fn empty_document_fits() {
        let page = make_page("", &Options::default(), 80, 5);
        assert_eq!(page.content_fits_on_screen(), true);
    }
}
