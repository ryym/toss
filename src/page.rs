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
