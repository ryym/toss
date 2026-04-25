use crate::{
    document::Document,
    line::Row,
    options::Options,
    pager::{global_header::GlobalHeader, section_header::SectionHeader, viewport::Viewport},
    screen::{Direction, Scroll},
};

mod global_header;
mod rows;
mod section_header;
mod viewport;

#[derive(Debug)]
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
    Scroll(Scroll),
    None,
}

#[derive(Debug)]
pub struct PageSnapshot<'pager> {
    pub global_header: &'pager [Row],
    pub section_header: &'pager [Row],
    pub content: &'pager [Row],
    pub height: usize,
    pub last_update: PageUpdate,
}

impl<'pager> PageSnapshot<'pager> {
    pub fn total_header_height(&self) -> usize {
        self.global_header.len() + self.section_header.len()
    }

    pub fn viewport_height(&self) -> usize {
        self.total_header_height() + self.content.len()
    }
}

/// Centrally manages the pagination state.
/// [`Pager`] reads the rows that fit in the display area from [`Document`] and shows them
/// together with the status line. The whole display area is called the page, and the part
/// that shows rows of [`Document`] lines in particular is called the viewport.
/// Depending on the configuration, a global header and a section header may be pinned at
/// the top of the viewport.
///
/// Internally the following structs manage the content displayed in the headers and viewport:
/// - Global header: [`GlobalHeader`]
/// - Section header: [`SectionHeader`]
/// - Viewport: [`Viewport`]
///
/// [`Viewport`] is unaware of headers and just holds a specific range of [`Document`] as
/// directed by [`Pager`]. The header rows managed by [`GlobalHeader`] and [`SectionHeader`]
/// are rendered as if overlaid on top of [`Viewport`].
/// With this overlay approach, [`Viewport`] can manage its rows independently,
/// without being affected by header content or height.
/// The role of [`Pager`] is to maintain this overlay correctly while applying the requested
/// operations to update the page state.
/// [`Pager`] only holds the state but does not write anything to the screen itself.
pub struct Pager {
    doc: Document,
    global_header: GlobalHeader,
    section_header: SectionHeader,
    viewport: Viewport,
    last_update: PageUpdate,
}

impl Pager {
    pub fn new(
        mut doc: Document,
        options: Options,
        screen_width: usize,
        screen_height: usize,
    ) -> Self {
        let size = ViewportSize::new(screen_width, screen_height);
        let global_header = GlobalHeader::new(&mut doc, &size, options.header);
        let mut section_header = SectionHeader::new(options.section, &size, global_header.height());
        section_header.resolve(&mut doc, 0);
        let viewport = Viewport::new(&mut doc, size);
        Self {
            doc,
            global_header,
            section_header,
            viewport,
            last_update: PageUpdate::Full,
        }
    }

    pub fn doc_mut(&mut self) -> &mut Document {
        &mut self.doc
    }

    pub fn snapshot<'pager>(&'pager mut self) -> (PageSnapshot<'pager>, &'pager mut Document) {
        let snapshot = PageSnapshot {
            global_header: self.global_header.rows(),
            section_header: self.section_header.rows(),
            content: &self.viewport.rows()[self.total_header_height()..],
            height: self.viewport.size().height,
            last_update: self.last_update,
        };
        self.last_update = PageUpdate::None;
        (snapshot, &mut self.doc)
    }

    pub fn total_header_height(&self) -> usize {
        self.global_header.height() + self.section_header.height()
    }

    /// Returns the height of the display area (the number of rows) excluding the header region.
    pub fn content_height(&self) -> usize {
        self.viewport.rows().len() - self.total_header_height()
    }

    /// Returns the rows that form a contiguous range within the viewport.
    /// When headers exist, they are included only if the header region and the content region
    /// are adjacent in the document; otherwise they are excluded.
    /// For example, if the section header shows lines 3-5 of [`Document`] and the content
    /// shows lines 6-30, the rows for lines 3-30 are returned.
    /// If a global header also exists at lines 1-2, the global header is included as well.
    /// However, if the content starts at line 7 or later (not adjacent), only the content rows
    /// are returned.
    pub fn contiguous_rows(&self) -> &[Row] {
        &self.viewport.rows()[self.contiguous_top_row_index()..]
    }

    fn contiguous_top_row_index(&self) -> usize {
        let rows = self.viewport.rows();
        if let Some(row) = self.global_header.rows().first()
            && row == &rows[0]
        {
            return 0;
        }
        if let Some(row) = self.section_header.rows().first()
            && row.line_index() == rows[self.global_header.rows().len()].line_index()
        {
            return self.global_header.rows().len();
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

    /// Resize the page to fit the new dimensions.
    pub fn resize(&mut self, screen_width: usize, screen_height: usize) {
        let size = ViewportSize::new(screen_width, screen_height);
        self.global_header.resize(&mut self.doc, &size);
        self.section_header
            .resize(&mut self.doc, &size, self.global_header.height());
        self.viewport.resize(&mut self.doc, size);
        self.last_update = PageUpdate::Full;
    }

    /// Move the page so that the specified line comes to the top.
    /// - If the specified line is within the global header, jump to the start of the document.
    /// - If the specified line is within any section header, move so that it comes to the top.
    /// - Otherwise, move so that the specified line comes right after the headers.
    pub fn jump_to(&mut self, mut line_index: usize) {
        if self.global_header.contains(line_index) {
            line_index = 0;
        }

        // Remember the position before the move so we can determine the final scroll amount.
        let prev_viewport_top = self.viewport.rows()[0].clone();
        let prev_line_pos = self.viewport.row_index(line_index, 0);

        self.section_header.resolve(&mut self.doc, line_index);

        let jump_offset = if self.section_header.contains(line_index) {
            self.global_header.height()
        } else {
            self.total_header_height()
        };
        let new_line_pos = self
            .viewport
            .jump_to(&mut self.doc, line_index, jump_offset);

        if prev_viewport_top < self.viewport.rows()[0] {
            // If this is a downward jump and the destination was within the original viewport,
            // we can treat this update as a scroll rather than a jump.
            self.last_update = if let Some(prev_line_pos) = prev_line_pos {
                let num_rows = new_line_pos.abs_diff(prev_line_pos);
                PageUpdate::Scroll(Scroll {
                    direction: Direction::Down,
                    num_rows,
                })
            } else {
                PageUpdate::Full
            };
        } else {
            // If this is an upward jump and the original top row of the viewport is still within
            // the new viewport, we can treat this update as a scroll rather than a jump.
            let prev_viewport_top_new_pos = self.viewport.row_index(
                prev_viewport_top.line_index(),
                prev_viewport_top.wrap_index(),
            );
            self.last_update = if let Some(pos) = prev_viewport_top_new_pos {
                PageUpdate::Scroll(Scroll {
                    direction: Direction::Up,
                    num_rows: pos,
                })
            } else {
                PageUpdate::Full
            };
        }
    }

    /// Jump to the end of the document so that the last line is at the bottom.
    pub fn jump_to_end(&mut self) {
        self.viewport.jump_to_end(&mut self.doc);

        let top_line_index = self.viewport.rows()[0].line_index();
        self.section_header.resolve(&mut self.doc, top_line_index);
        self.push_up_section_header_if_needed();

        self.last_update = PageUpdate::Full;
    }

    /// Scroll by the given number of rows (positive = down, negative = up).
    /// Returns the number of rows scrolled.
    /// This may be less than `num_rows` If there is limited room to scroll.
    pub fn scroll(&mut self, num_rows: i32) -> usize {
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
        self.last_update = PageUpdate::Scroll(Scroll {
            direction: if num_rows < 0 {
                Direction::Up
            } else {
                Direction::Down
            },
            num_rows: actual_scroll_rows,
        });
        actual_scroll_rows
    }

    fn scroll_up(&mut self, num_rows: usize) -> usize {
        let rows_scrolled = self.viewport.scroll_up(&mut self.doc, num_rows);

        // Check the section header status to update it as needed.
        let section_header_start = match self.section_header.start_line_index() {
            Some(idx) => idx,
            // If there is no current section, scrolling upward cannot newly reveal one, so do nothing.
            None => return rows_scrolled,
        };

        // If the new top row is above the current section header, search for a section header above it.
        let top_line = self.viewport.rows()[self.global_header.height()].line_index();
        if top_line < section_header_start {
            self.section_header.resolve(&mut self.doc, top_line);
        }
        self.push_up_section_header_if_needed();

        rows_scrolled
    }

    fn scroll_down(&mut self, num_rows: usize) -> usize {
        let prev_top_line = self.viewport.rows()[self.global_header.height()].line_index();
        let rows_scrolled = self.viewport.scroll_down(&mut self.doc, num_rows);
        let top_line = self.viewport.rows()[self.global_header.height()].line_index();

        // If a new section header exists within the moved range, replace the current one with it.
        self.section_header
            .resolve_if_found(&mut self.doc, prev_top_line..(top_line + 1));
        self.push_up_section_header_if_needed();

        rows_scrolled
    }

    /// Look for another section header underneath the current section header overlay,
    /// and if one is found (i.e. a section transition is in progress), adjust the offset of
    /// the current section so that the new section header becomes visible.
    fn push_up_section_header_if_needed(&mut self) {
        let current_start_line = match self.section_header.start_line_index() {
            Some(i) => i,
            None => return,
        };

        let overlay_height = self.global_header.height() + self.section_header.full_height();
        let mut other_section_start = overlay_height;
        let rows_under_section_header = self
            .viewport
            .rows()
            .iter()
            .enumerate()
            .take(overlay_height)
            .skip(self.global_header.height());
        for (i, row) in rows_under_section_header {
            if row.wrap_index() != 0 || row.line_index() == current_start_line {
                continue;
            }
            if self
                .section_header
                .is_header(&mut self.doc, row.line_index())
            {
                other_section_start = i;
                break;
            }
        }
        let push_up = overlay_height.saturating_sub(other_section_start);
        self.section_header.push_up(push_up);
    }
}
