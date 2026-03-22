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
        let section_index =
            section.map(|opts| SectionIndex::new(opts.pattern.clone(), opts.header_lines));
        Self {
            fixed_lines,
            section_index,
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
        viewport_top_wrap: usize,
        sync_section: bool,
    ) -> Vec<ScreenRow> {
        if sync_section && let Some(ref mut index) = self.section_index {
            index.find_section(doc, viewport_top);
        }

        let mut rows = self.resolve_fixed(doc, width);
        let fixed_row_count = rows.len();
        let mut use_overlay = false;

        if let Some(ref index) = self.section_index
            && let Some(section_start) = index.current_section()
        {
            let n = index.header_lines();
            use_overlay = n > 1;

            // Build full header block screen rows.
            let effective_start = section_start.max(self.fixed_lines);
            let mut block_rows = Vec::new();
            for i in effective_start..section_start + n {
                if let Some(line) = doc.line(i) {
                    for w in 0..line.row_count(width) {
                        block_rows.push(ScreenRow {
                            line_index: i,
                            wrap_index: w,
                        });
                    }
                }
            }

            let display_rows = if use_overlay {
                // Push-up: limit overlay to the screen row distance to the next section.
                let max_rows = index
                    .find_next_section_row_distance(
                        doc,
                        viewport_top,
                        viewport_top_wrap,
                        block_rows.len(),
                        width,
                    )
                    .unwrap_or(block_rows.len());
                max_rows.min(block_rows.len())
            } else {
                block_rows.len()
            };

            // Show the LAST display_rows of the block
            // (bottom rows remain while top rows get pushed off).
            let skip = block_rows.len() - display_rows;
            rows.extend_from_slice(&block_rows[skip..]);
        }

        let section_row_count = rows.len() - fixed_row_count;
        self.cached_overlay = if use_overlay { section_row_count } else { 0 };

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
    /// Number of lines per section header block.
    header_lines: usize,
    /// The section-start line for the current sticky header, if any.
    cached_section: Option<usize>,
}

impl SectionIndex {
    fn new(pattern: Regex, header_lines: usize) -> Self {
        Self {
            pattern,
            header_lines,
            cached_section: None,
        }
    }

    fn header_lines(&self) -> usize {
        self.header_lines
    }

    /// Returns the current cached section start line.
    fn current_section(&self) -> Option<usize> {
        self.cached_section
    }

    /// Given a pattern match at `match_line`, walk backward to find the true
    /// section start. A match might fall within the header block of an earlier
    /// section, in which case that earlier section is the real start.
    fn resolve_section_start(&self, doc: &mut Document, match_line: usize) -> usize {
        let mut current = match_line;
        loop {
            let check_start = current.saturating_sub(self.header_lines - 1);
            let earlier = (check_start..current).find(|&j| {
                doc.line(j)
                    .is_some_and(|line| self.pattern.is_match(line.plain()))
            });
            match earlier {
                Some(j) => current = j,
                None => return current,
            }
        }
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
                let start = self.resolve_section_start(doc, i);
                self.cached_section = Some(start);
                return Some(start);
            }
        }
        self.cached_section = None;
        None
    }

    /// Find the screen row distance from `viewport_top` to the next section start.
    /// Scans forward, accumulating screen rows per line (accounting for wrapping).
    /// Skips matches within the current section's header block.
    /// Returns None if no section is found within `max_rows` screen rows.
    fn find_next_section_row_distance(
        &self,
        doc: &mut Document,
        viewport_top: usize,
        viewport_top_wrap: usize,
        max_rows: usize,
        width: usize,
    ) -> Option<usize> {
        let skip_until = self
            .cached_section
            .map(|s| s + self.header_lines)
            .unwrap_or(0);
        let mut row_count = 0;
        let mut line_idx = viewport_top;
        while row_count < max_rows {
            let line = doc.line(line_idx)?;
            if line_idx >= skip_until && self.pattern.is_match(line.plain()) {
                return Some(row_count);
            }
            let rows = line.row_count(width);
            // For the first line, only count rows from the current wrap position.
            if line_idx == viewport_top {
                row_count += rows - viewport_top_wrap;
            } else {
                row_count += rows;
            }
            line_idx += 1;
        }
        None
    }

    /// On scroll down, check if any new sections appeared in the scrolled
    /// range. Skips matches within the current section's header block.
    fn update_on_scroll_down(&mut self, doc: &mut Document, old_top: usize, new_top: usize) {
        let mut skip_until = self
            .cached_section
            .map(|s| s + self.header_lines)
            .unwrap_or(0);
        for i in old_top..new_top {
            if i < skip_until {
                continue;
            }
            if let Some(line) = doc.line(i)
                && self.pattern.is_match(line.plain())
            {
                self.cached_section = Some(i);
                skip_until = i + self.header_lines;
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
                    let start = self.resolve_section_start(doc, i);
                    self.cached_section = Some(start);
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
        let mut idx = SectionIndex::new(Regex::new("^#").unwrap(), 1);
        assert_eq!(idx.find_section(&mut doc, 2), None);
        assert_eq!(idx.current_section(), None);
    }

    #[test]
    fn find_section_basic() {
        let mut doc = make_doc("# Section A\nline 1\nline 2\n# Section B\nline 3");
        let mut idx = SectionIndex::new(Regex::new("^# ").unwrap(), 1);

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
        let mut idx = SectionIndex::new(Regex::new("^# ").unwrap(), 1);

        // viewport_top=1: section_start=0 < 1, sticky
        assert_eq!(idx.find_section(&mut doc, 1), Some(0));

        // viewport_top=2: still sticky
        assert_eq!(idx.find_section(&mut doc, 2), Some(0));
    }

    #[test]
    fn scroll_down_updates_section() {
        let mut doc = make_doc("# A\nline\n# B\nline\nline");
        let mut idx = SectionIndex::new(Regex::new("^# ").unwrap(), 1);

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
        let mut idx = SectionIndex::new(Regex::new("^# ").unwrap(), 1);

        // Scroll from 0 to 1: line 0 matches, 0 < 1 → sticky
        idx.update_on_scroll_down(&mut doc, 0, 1);
        assert_eq!(idx.current_section(), Some(0));
    }

    #[test]
    fn scroll_up_reverts_to_previous_section() {
        let mut doc = make_doc("# A\nline\n# B\nline\nline");
        let mut idx = SectionIndex::new(Regex::new("^# ").unwrap(), 1);
        idx.cached_section = Some(2); // Currently on section B

        // Scroll up so new_top=2: section_start=2 >= new_top=2, no longer sticky.
        // Scan backward from 1: line 1 (no), line 0 (yes) → Section A.
        idx.update_on_scroll_up(&mut doc, 2);
        assert_eq!(idx.current_section(), Some(0));
    }

    #[test]
    fn scroll_up_no_previous_section() {
        let mut doc = make_doc("line\nline\n# B\nline");
        let mut idx = SectionIndex::new(Regex::new("^# ").unwrap(), 1);
        idx.cached_section = Some(2);

        // Scroll up past section B: section_start=2 >= new_top=2
        idx.update_on_scroll_up(&mut doc, 2);
        assert_eq!(idx.current_section(), None);
    }

    // --- Block-skip tests ---

    #[test]
    fn find_section_skips_match_within_block() {
        // header_lines=3: section at line 0 owns block [0,1,2].
        // Line 2 matches but is within line 0's block.
        let mut doc = make_doc("# A\nline\n## sub\nline\nline");
        let mut idx = SectionIndex::new(Regex::new("^#").unwrap(), 3);

        // Scanning backward from viewport_top=3: finds ## sub at line 2,
        // then resolves to line 0 (the true section start).
        assert_eq!(idx.find_section(&mut doc, 3), Some(0));
    }

    #[test]
    fn scroll_down_skips_match_within_block() {
        // header_lines=3: line 0 starts section, block [0,1,2].
        // Line 2 matches but should be skipped.
        let mut doc = make_doc("# A\nline\n## sub\nline\n# B\nline");
        let mut idx = SectionIndex::new(Regex::new("^#").unwrap(), 3);

        // Scroll from 0 to 3: line 0 matches (section), line 2 is within block → skip
        idx.update_on_scroll_down(&mut doc, 0, 3);
        assert_eq!(idx.current_section(), Some(0));

        // Scroll from 3 to 5: line 4 (# B) matches and is outside block → new section
        idx.update_on_scroll_down(&mut doc, 3, 5);
        assert_eq!(idx.current_section(), Some(4));
    }

    #[test]
    fn scroll_up_resolves_through_block() {
        // header_lines=3: line 0 starts section, block [0,1,2].
        // Line 2 matches. When scrolling up to find previous section,
        // the backward scan should resolve line 2 → line 0.
        let mut doc = make_doc("# A\nline\n## sub\nline\n# B\nline");
        let mut idx = SectionIndex::new(Regex::new("^#").unwrap(), 3);
        idx.cached_section = Some(4); // Currently on section B

        // Scroll up so new_top=4: section B no longer sticky.
        // Backward scan finds ## sub (line 2), resolves to # A (line 0).
        idx.update_on_scroll_up(&mut doc, 4);
        assert_eq!(idx.current_section(), Some(0));
    }
}
