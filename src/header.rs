use regex::Regex;

use crate::document::Document;
use crate::options::SectionOptions;
use crate::viewport::ScreenRow;

/// Manages sticky header lines pinned at the top of the screen.
pub struct Header {
    /// Number of logical lines to pin from the top of the document.
    fixed_lines: usize,
    /// Section index for dynamic sticky headers.
    section_index: Option<SectionIndex>,
    /// Number of lines per section header block.
    section_header_lines: usize,
    /// Cached: number of viewport screen rows overlaid by the section header.
    /// For multi-line section headers (N>=2), the section header overlays
    /// viewport rows instead of resizing the viewport. For single-line
    /// headers (N=1), this is always 0 (resize model is used).
    cached_overlay: usize,
}

impl Header {
    /// Create a new header with the given number of fixed lines
    /// and optional section configuration.
    pub fn new(fixed_lines: usize, section: Option<&SectionOptions>) -> Self {
        let (section_index, section_header_lines) = match section {
            Some(opts) => (
                Some(SectionIndex::new(opts.pattern.clone())),
                opts.header_lines,
            ),
            None => (None, 0),
        };
        Self {
            fixed_lines,
            section_index,
            section_header_lines,
            cached_overlay: 0,
        }
    }

    /// The minimum line index that the viewport may start from.
    /// Lines below this are reserved for the fixed header display.
    pub fn min_top_line(&self) -> usize {
        self.fixed_lines
    }

    /// Returns the current sticky section start line, if any.
    pub fn current_section(&self) -> Option<usize> {
        self.section_index
            .as_ref()
            .and_then(|idx| idx.current_section())
    }

    /// Number of viewport screen rows overlaid by the section header.
    /// For multi-line section headers (N>=2), the section header overlays
    /// viewport rows. For single-line headers (N=1), this is 0.
    pub fn section_overlay(&self) -> usize {
        self.cached_overlay
    }

    /// Update the section index cache after a scroll operation.
    pub fn update_section_on_scroll(
        &mut self,
        doc: &mut Document,
        old_top: usize,
        new_top: usize,
        is_down: bool,
    ) {
        if let Some(ref mut index) = self.section_index {
            if is_down {
                index.update_on_scroll_down(doc, old_top, new_top);
            } else {
                index.update_on_scroll_up(doc, new_top);
            }
        }
    }

    /// Resolve the header rows to display, accounting for line wrapping.
    /// When `sync_section` is true, runs a backward scan to synchronize the
    /// section index cache (used on full redraws and jumps).
    pub fn resolve(
        &mut self,
        doc: &mut Document,
        width: usize,
        viewport_top: usize,
        sync_section: bool,
    ) -> Vec<ScreenRow> {
        if sync_section && let Some(ref mut index) = self.section_index {
            index.find_section(doc, viewport_top);
        }

        let mut rows = self.resolve_fixed(doc, width);
        let fixed_row_count = rows.len();

        if let Some(ref index) = self.section_index
            && let Some(section_start) = index.current_section()
        {
            let n = self.section_header_lines;
            let display_lines = if n > 1 {
                // Push-up: reduce display lines when the next section approaches.
                index
                    .find_next_section_distance(doc, viewport_top, n)
                    .unwrap_or(n)
            } else {
                n
            };

            if display_lines > 0 {
                // For multi-line headers, show the LAST display_lines of the block
                // (bottom lines remain while top lines get pushed off).
                let block_start = section_start + (n - display_lines);
                let block_end = section_start + n;
                // Skip lines that overlap with the fixed header.
                let effective_start = block_start.max(self.fixed_lines);
                for i in effective_start..block_end {
                    if let Some(line) = doc.line(i) {
                        for w in 0..line.row_count(width) {
                            rows.push(ScreenRow {
                                line_index: i,
                                wrap_index: w,
                            });
                        }
                    }
                }
            }
        }

        let section_row_count = rows.len() - fixed_row_count;
        self.cached_overlay = if self.section_header_lines > 1 {
            section_row_count
        } else {
            0
        };

        rows
    }

    /// Returns the height of the fixed header portion only (for initial layout).
    pub fn resolve_fixed_height(&self, doc: &mut Document, width: usize) -> usize {
        self.resolve_fixed(doc, width).len()
    }

    /// Resolve only the fixed header rows.
    fn resolve_fixed(&self, doc: &mut Document, width: usize) -> Vec<ScreenRow> {
        let mut rows = vec![];
        for i in 0..self.fixed_lines {
            if let Some(line) = doc.line(i) {
                for w in 0..line.row_count(width) {
                    rows.push(ScreenRow {
                        line_index: i,
                        wrap_index: w,
                    });
                }
            }
        }
        rows
    }
}

/// Tracks section positions for dynamic sticky headers.
///
/// A section starts at a line matching the configured regex pattern.
/// The section header block spans `header_lines` lines from that start.
/// A section becomes "sticky" when its start line scrolls above the
/// viewport (`section_start < viewport_top`).
struct SectionIndex {
    pattern: Regex,
    /// The section-start line for the current sticky header, if any.
    cached_section: Option<usize>,
}

impl SectionIndex {
    fn new(pattern: Regex) -> Self {
        Self {
            pattern,
            cached_section: None,
        }
    }

    /// Returns the current cached section start line.
    fn current_section(&self) -> Option<usize> {
        self.cached_section
    }

    /// Scan backward from `viewport_top` to find the nearest section
    /// whose start is above the viewport (`section_start < viewport_top`).
    fn find_section(&mut self, doc: &mut Document, viewport_top: usize) -> Option<usize> {
        if viewport_top == 0 {
            self.cached_section = None;
            return None;
        }
        for i in (0..viewport_top).rev() {
            if let Some(line) = doc.line(i)
                && self.pattern.is_match(line.plain())
            {
                self.cached_section = Some(i);
                return Some(i);
            }
        }
        self.cached_section = None;
        None
    }

    /// Find the distance from `viewport_top` to the next section start.
    /// Only scans up to `max_distance` lines. Returns None if no section
    /// is found within that range.
    fn find_next_section_distance(
        &self,
        doc: &mut Document,
        viewport_top: usize,
        max_distance: usize,
    ) -> Option<usize> {
        for d in 0..max_distance {
            if let Some(line) = doc.line(viewport_top + d)
                && self.pattern.is_match(line.plain())
            {
                return Some(d);
            }
        }
        None
    }

    /// On scroll down, check if any new sections appeared in the scrolled
    /// range. Any section at `section_start < new_top` qualifies as sticky.
    fn update_on_scroll_down(&mut self, doc: &mut Document, old_top: usize, new_top: usize) {
        for i in old_top..new_top {
            if let Some(line) = doc.line(i)
                && self.pattern.is_match(line.plain())
            {
                self.cached_section = Some(i);
            }
        }
    }

    /// On scroll up, check if the cached section is no longer sticky
    /// (`section_start >= new_top`). If so, scan backward for an earlier one.
    fn update_on_scroll_up(&mut self, doc: &mut Document, new_top: usize) {
        let Some(section_start) = self.cached_section else {
            return;
        };
        if section_start >= new_top {
            self.cached_section = None;
            if new_top == 0 {
                return;
            }
            for i in (0..new_top).rev() {
                if let Some(line) = doc.line(i)
                    && self.pattern.is_match(line.plain())
                {
                    self.cached_section = Some(i);
                    return;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_doc(content: &str) -> Document {
        Document::from_string(content.to_string())
    }

    #[test]
    fn find_section_no_match() {
        let mut doc = make_doc("line 1\nline 2\nline 3");
        let mut idx = SectionIndex::new(Regex::new("^#").unwrap());
        assert_eq!(idx.find_section(&mut doc, 2), None);
        assert_eq!(idx.current_section(), None);
    }

    #[test]
    fn find_section_basic() {
        let mut doc = make_doc("# Section A\nline 1\nline 2\n# Section B\nline 3");
        let mut idx = SectionIndex::new(Regex::new("^# ").unwrap());

        // viewport_top=2: section at line 0 qualifies (0 < 2)
        assert_eq!(idx.find_section(&mut doc, 2), Some(0));

        // viewport_top=4: section at line 3 qualifies (3 < 4)
        assert_eq!(idx.find_section(&mut doc, 4), Some(3));
    }

    #[test]
    fn find_section_partially_visible() {
        // With header_lines=2, section at line 0 becomes sticky at viewport_top=1
        // (section_start=0 < 1).
        let mut doc = make_doc("# Sec\ncontinued\nline 2\nline 3");
        let mut idx = SectionIndex::new(Regex::new("^# ").unwrap());

        // viewport_top=1: section_start=0 < 1, sticky
        assert_eq!(idx.find_section(&mut doc, 1), Some(0));

        // viewport_top=2: still sticky
        assert_eq!(idx.find_section(&mut doc, 2), Some(0));
    }

    #[test]
    fn scroll_down_updates_section() {
        let mut doc = make_doc("# A\nline\n# B\nline\nline");
        let mut idx = SectionIndex::new(Regex::new("^# ").unwrap());

        // Initially no section
        assert_eq!(idx.current_section(), None);

        // Scroll from top=0 to top=1: line 0 (# A) scrolled past (0 < 1)
        idx.update_on_scroll_down(&mut doc, 0, 1);
        assert_eq!(idx.current_section(), Some(0));

        // Scroll from top=1 to top=3: line 2 (# B) scrolled past (2 < 3)
        idx.update_on_scroll_down(&mut doc, 1, 3);
        assert_eq!(idx.current_section(), Some(2));
    }

    #[test]
    fn scroll_down_multi_line_header() {
        let mut doc = make_doc("# A\ncont\nline\nline");
        let mut idx = SectionIndex::new(Regex::new("^# ").unwrap());

        // Scroll from 0 to 1: line 0 matches, 0 < 1 → sticky
        idx.update_on_scroll_down(&mut doc, 0, 1);
        assert_eq!(idx.current_section(), Some(0));
    }

    #[test]
    fn scroll_up_reverts_to_previous_section() {
        let mut doc = make_doc("# A\nline\n# B\nline\nline");
        let mut idx = SectionIndex::new(Regex::new("^# ").unwrap());
        idx.cached_section = Some(2); // Currently on section B

        // Scroll up so new_top=2: section_start=2 >= new_top=2, no longer sticky.
        // Scan backward from 1: line 1 (no), line 0 (yes) → Section A.
        idx.update_on_scroll_up(&mut doc, 2);
        assert_eq!(idx.current_section(), Some(0));
    }

    #[test]
    fn scroll_up_no_previous_section() {
        let mut doc = make_doc("line\nline\n# B\nline");
        let mut idx = SectionIndex::new(Regex::new("^# ").unwrap());
        idx.cached_section = Some(2);

        // Scroll up past section B: section_start=2 >= new_top=2
        idx.update_on_scroll_up(&mut doc, 2);
        assert_eq!(idx.current_section(), None);
    }
}
