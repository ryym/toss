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
                index.update_on_scroll_down(doc, old_top, new_top, self.section_header_lines);
            } else {
                index.update_on_scroll_up(doc, new_top, self.section_header_lines);
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
            index.find_section(doc, viewport_top, self.section_header_lines);
        }

        let mut rows = self.resolve_fixed(doc, width);

        if let Some(ref index) = self.section_index
            && let Some(section_start) = index.current_section()
        {
            // Skip lines that overlap with the fixed header.
            let effective_start = section_start.max(self.fixed_lines);
            let section_end = section_start + self.section_header_lines;
            for i in effective_start..section_end {
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
/// The block becomes "sticky" (displayed at the top) when it scrolls
/// entirely above the viewport.
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

    /// Scan backward from `viewport_top` to find the nearest section whose
    /// header block is entirely above the viewport (sticky condition:
    /// `section_start + header_lines <= viewport_top`).
    /// Updates `cached_section`.
    fn find_section(
        &mut self,
        doc: &mut Document,
        viewport_top: usize,
        header_lines: usize,
    ) -> Option<usize> {
        // We need section_start + header_lines <= viewport_top,
        // so the latest possible section_start is viewport_top - header_lines.
        if header_lines == 0 || viewport_top < header_lines {
            self.cached_section = None;
            return None;
        }
        let max_start = viewport_top - header_lines;
        for i in (0..=max_start).rev() {
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

    /// Check lines around the scroll range for section matches that now
    /// satisfy the sticky condition (`section_start + header_lines <= new_top`).
    /// We look back `header_lines - 1` rows before `old_top` to catch sections
    /// whose header block was partially visible before but is now fully above.
    fn update_on_scroll_down(
        &mut self,
        doc: &mut Document,
        old_top: usize,
        new_top: usize,
        header_lines: usize,
    ) {
        if header_lines == 0 {
            return;
        }
        let scan_start = old_top.saturating_sub(header_lines - 1);
        for i in scan_start..new_top {
            if let Some(line) = doc.line(i)
                && self.pattern.is_match(line.plain())
                && i + header_lines <= new_top
            {
                self.cached_section = Some(i);
            }
        }
    }

    /// On scroll up, check if the cached section's header block has come
    /// back into the viewport. If so, scan backward for the previous section.
    fn update_on_scroll_up(&mut self, doc: &mut Document, new_top: usize, header_lines: usize) {
        if header_lines == 0 {
            self.cached_section = None;
            return;
        }
        let Some(section_start) = self.cached_section else {
            return;
        };
        // The header block is visible again if section_start + header_lines > new_top.
        if section_start + header_lines > new_top {
            // Scan backward from section_start to find the previous section.
            self.cached_section = None;
            if section_start == 0 {
                return;
            }
            let max_start = if new_top >= header_lines {
                new_top - header_lines
            } else {
                return;
            };
            for i in (0..section_start.min(max_start + 1)).rev() {
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
        assert_eq!(idx.find_section(&mut doc, 2, 1), None);
        assert_eq!(idx.current_section(), None);
    }

    #[test]
    fn find_section_basic() {
        let mut doc = make_doc("# Section A\nline 1\nline 2\n# Section B\nline 3");
        let mut idx = SectionIndex::new(Regex::new("^# ").unwrap());

        // viewport_top=2, header_lines=1: section at line 0 qualifies (0+1<=2)
        assert_eq!(idx.find_section(&mut doc, 2, 1), Some(0));

        // viewport_top=4: section at line 3 qualifies (3+1<=4)
        assert_eq!(idx.find_section(&mut doc, 4, 1), Some(3));
    }

    #[test]
    fn find_section_header_block_partially_visible() {
        // With header_lines=2, section at line 0 needs viewport_top >= 2 to be sticky.
        let mut doc = make_doc("# Sec\ncontinued\nline 2\nline 3");
        let mut idx = SectionIndex::new(Regex::new("^# ").unwrap());

        // viewport_top=1: block [0,1] is partially visible, not sticky
        assert_eq!(idx.find_section(&mut doc, 1, 2), None);

        // viewport_top=2: block [0,1] is entirely above viewport, sticky
        assert_eq!(idx.find_section(&mut doc, 2, 2), Some(0));
    }

    #[test]
    fn scroll_down_updates_section() {
        let mut doc = make_doc("# A\nline\n# B\nline\nline");
        let mut idx = SectionIndex::new(Regex::new("^# ").unwrap());

        // Initially no section
        assert_eq!(idx.current_section(), None);

        // Scroll from top=0 to top=1: line 0 (# A) scrolled past, 0+1<=1
        idx.update_on_scroll_down(&mut doc, 0, 1, 1);
        assert_eq!(idx.current_section(), Some(0));

        // Scroll from top=1 to top=3: line 2 (# B) scrolled past, 2+1<=3
        idx.update_on_scroll_down(&mut doc, 1, 3, 1);
        assert_eq!(idx.current_section(), Some(2));
    }

    #[test]
    fn scroll_down_does_not_update_if_block_partially_visible() {
        let mut doc = make_doc("# A\ncont\nline\nline");
        let mut idx = SectionIndex::new(Regex::new("^# ").unwrap());

        // header_lines=2, scroll from 0 to 1: line 0 matches but 0+2 > 1
        idx.update_on_scroll_down(&mut doc, 0, 1, 2);
        assert_eq!(idx.current_section(), None);

        // Scroll from 1 to 2: now 0+2 <= 2
        idx.update_on_scroll_down(&mut doc, 1, 2, 2);
        assert_eq!(idx.current_section(), Some(0));
    }

    #[test]
    fn scroll_up_reverts_to_previous_section() {
        let mut doc = make_doc("# A\nline\n# B\nline\nline");
        let mut idx = SectionIndex::new(Regex::new("^# ").unwrap());
        idx.cached_section = Some(2); // Currently on section B

        // Scroll up so new_top=2: block [2,2+1=3) still above? 2+1=3 > 2, so visible
        idx.update_on_scroll_up(&mut doc, 2, 1);
        // Should revert to section A (line 0), but 0+1<=2 is needed.
        // new_top=2, max_start=2-1=1, scan 0..min(2,2)=0..2 rev: check line 1 (no), line 0 (yes)
        assert_eq!(idx.current_section(), Some(0));
    }

    #[test]
    fn scroll_up_no_previous_section() {
        let mut doc = make_doc("line\nline\n# B\nline");
        let mut idx = SectionIndex::new(Regex::new("^# ").unwrap());
        idx.cached_section = Some(2);

        // Scroll up past section B
        idx.update_on_scroll_up(&mut doc, 2, 1);
        assert_eq!(idx.current_section(), None);
    }
}
