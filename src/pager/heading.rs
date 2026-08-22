use std::ops::Range;

use crate::{
    document::Document,
    line::Row,
    options::HeadingOptions,
    pager::{ViewportSize, rows},
};

/// Manages the sticky heading. Unlike the global header which always shows fixed lines,
/// a line matching the given pattern dynamically becomes the heading and is shown pinned
/// below the global header. The behavior is similar to CSS `position: sticky`;
/// the displayed heading changes as the user scrolls.
/// The nearest line matching the pattern among the lines above the first line of the viewport
/// becomes the start line of the heading.
#[derive(Debug)]
pub(super) struct Heading {
    config: HeadingConfig,
    options: Option<HeadingOptions>,
    current: Option<HeadingState>,
}

impl Heading {
    pub fn new(
        options: Option<HeadingOptions>,
        size: &ViewportSize,
        global_header_height: usize,
        global_header_num_lines: usize,
    ) -> Self {
        Self {
            config: HeadingConfig::new(size, global_header_height, global_header_num_lines),
            options,
            current: None,
        }
    }

    /// Determine if the line at `line_index` is the start line of a heading.
    ///
    /// Example: with `toss --heading '^#' --heading-lines 2`
    /// ```text
    /// # title     => is_heading_start: true
    /// sub title   => is_heading_start: false (not a start line)
    /// other line  => is_heading_start: false
    /// ```
    ///
    /// Even when a line matches the heading pattern,
    /// if there is another line that also matches the pattern within the `--heading-lines` lines,
    /// the first line is NOT treated as a heading.
    ///
    /// Example: with `toss --heading '^#' --heading-lines 2`
    /// ```text
    /// # title 1    => Not a heading as there is `## title 2`
    /// ## title 2   => Not a heading as there is `### title 3`
    /// ### title 3  => A heading and is_heading_start is true
    /// sentence 1   => A part of the heading but not a start line (is_heading_start is false)
    /// sentence 2   => Not a heading
    /// ```
    pub fn is_heading_start(&self, doc: &mut Document, line_index: usize) -> bool {
        match &self.options {
            Some(options) => is_heading_start(doc, line_index, options),
            None => false,
        }
    }

    pub fn start_line_index(&self) -> Option<usize> {
        self.current.as_ref().map(|h| h.line_range.start)
    }

    /// The heading rows visible in the page.
    pub fn rows(&self) -> &[Row] {
        match &self.current {
            None => &[],
            Some(h) => &h.rows[h.offset..],
        }
    }

    /// The heading height visible in the page.
    pub fn height(&self) -> usize {
        self.rows().len()
    }

    /// The heading height without accounting for the offset set by [`Self::push_up`].
    pub fn full_height(&self) -> usize {
        match &self.current {
            None => 0,
            Some(h) => h.rows.len(),
        }
    }

    pub fn contains(&self, line_index: usize) -> bool {
        let rows = self.rows();
        !rows.is_empty()
            && rows[0].line_index() <= line_index
            && line_index <= rows[rows.len() - 1].line_index()
    }

    pub fn resize(
        &mut self,
        doc: &mut Document,
        size: &ViewportSize,
        global_header_height: usize,
        global_header_num_lines: usize,
    ) {
        self.config = HeadingConfig::new(size, global_header_height, global_header_num_lines);
        if let Some(h) = &mut self.current {
            h.rows = rows::from_lines(
                doc,
                self.config.width,
                h.line_range.clone(),
                self.config.max_heading_height,
            );
        }
    }

    /// Shift the heading by `num_rows` upward and hide the top portion.
    pub fn push_up(&mut self, num_rows: usize) {
        if let Some(h) = &mut self.current {
            assert!(
                num_rows <= h.rows.len(),
                "push up too large: {num_rows} > len {}",
                h.rows.len()
            );
            h.offset = num_rows;
        }
    }

    /// Find and set the heading nearest to the given `line_index`.
    /// If the line of `line_index` itself is a part of a heading, that heading is set.
    /// If no heading is found above `line_index`, the current heading is unset.
    pub fn resolve(&mut self, doc: &mut Document, line_index: usize) {
        self.current = self.find_heading(doc, self.config.min_line_index..(line_index + 1));
    }

    /// Same as [`Self::resolve`] but keep the current heading when no heading is found.
    pub fn resolve_if_found(&mut self, doc: &mut Document, line_index_range: Range<usize>) {
        if let Some(heading) = self.find_heading(doc, line_index_range) {
            self.current = Some(heading);
        }
    }

    fn find_heading(&self, doc: &mut Document, range: Range<usize>) -> Option<HeadingState> {
        let options = self.options.as_ref()?;
        if self.config.max_heading_height == 0 {
            return None;
        }

        // Search for the heading nearest to `range.end` within the range.
        let mut nearest = None;
        if range.end > range.start {
            for i in (range.start..range.end).rev() {
                if is_heading_start(doc, i, options) {
                    nearest = Some(i);
                    break;
                }
            }
        }
        let nearest = nearest?;

        let height = options.num_lines.min(self.config.max_heading_height);
        let line_range = nearest..(nearest + height);
        let rows = rows::from_lines(
            doc,
            self.config.width,
            line_range.clone(),
            self.config.max_heading_height,
        );
        Some(HeadingState {
            rows,
            line_range,
            offset: 0,
        })
    }
}

/// See [`Heading::is_heading_start`].
fn is_heading_start(doc: &mut Document, line_index: usize, options: &HeadingOptions) -> bool {
    match doc.line(line_index) {
        Some(line) if line.has_match(&options.pattern) => {}
        _ => return false,
    }
    // A line is treated as a heading if it matches the pattern and no other line within the
    // following `num_lines` lines also matches the pattern.
    for i in 1..options.num_lines {
        match doc.line(line_index + i) {
            Some(line) => {
                if line.has_match(&options.pattern) {
                    return false;
                }
            }
            None => return true,
        }
    }
    true
}

#[derive(Debug)]
struct HeadingConfig {
    /// The minimum line index that can be a heading.
    /// Lines below this index are never treated as headings, regardless of pattern matching.
    min_line_index: usize,
    max_heading_height: usize,
    width: usize,
}

impl HeadingConfig {
    fn new(
        size: &ViewportSize,
        global_header_height: usize,
        global_header_num_lines: usize,
    ) -> Self {
        // Reserve at least one non-heading row so the heading does not cover the entire viewport.
        let max_heading_height = size
            .height()
            .saturating_sub(global_header_height)
            .saturating_sub(1);
        Self {
            // Header lines are never heading candidates, regardless of how many rows they
            // render as.
            min_line_index: global_header_num_lines,
            max_heading_height,
            width: size.width(),
        }
    }
}

/// State representing a heading.
#[derive(Debug)]
struct HeadingState {
    line_range: Range<usize>,
    rows: Vec<Row>,
    /// Display `rows` starting from index `offset` in the viewport.
    /// Used when the heading transitions during scrolling.
    /// For example, when scrolling down and approaching the next heading,
    /// the current heading gradually increases its `offset` to disappear upward,
    /// and is replaced row by row by the new heading.
    offset: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::Document;
    use regex::Regex;

    fn opts(pattern: &str, num_lines: usize) -> HeadingOptions {
        HeadingOptions {
            pattern: Regex::new(pattern).unwrap(),
            num_lines,
        }
    }

    fn size(width: usize, height: usize) -> ViewportSize {
        ViewportSize { width, height }
    }

    #[test]
    fn no_options_means_no_heading_ever_resolved() {
        let mut doc = Document::from_string("# h\nfoo\n".into());
        let mut h = Heading::new(None, &size(10, 5), 0, 0);
        h.resolve(&mut doc, 1);
        assert!(h.start_line_index().is_none());
        assert!(h.rows().is_empty());
        assert_eq!(h.height(), 0);
        assert_eq!(h.full_height(), 0);
        assert!(!h.is_heading_start(&mut doc, 0));
    }

    #[test]
    fn is_heading_start_matches_lone_pattern_line() {
        let mut doc = Document::from_string("# h\nfoo\n".into());
        let h = Heading::new(Some(opts("^# ", 1)), &size(10, 5), 0, 0);
        assert!(h.is_heading_start(&mut doc, 0));
        assert!(!h.is_heading_start(&mut doc, 1));
    }

    #[test]
    fn is_heading_start_with_multi_line_window_takes_last_match() {
        // num_lines = 2: only the last match within a 2-line window is the heading.
        let mut doc = Document::from_string("# A\n# B\nfoo\nbar\n".into());
        let h = Heading::new(Some(opts("^# ", 2)), &size(10, 5), 0, 0);
        // Line 0 has another match (line 1) within the next num_lines-1 lines, so not a heading.
        assert!(!h.is_heading_start(&mut doc, 0));
        // Line 1 has no further matches within its window, so it counts as a heading.
        assert!(h.is_heading_start(&mut doc, 1));
    }

    #[test]
    fn resolve_finds_nearest_heading_above() {
        let mut doc = Document::from_string("# A\nx\ny\n# B\nz\n".into());
        let mut h = Heading::new(Some(opts("^# ", 1)), &size(10, 8), 0, 0);

        h.resolve(&mut doc, 4);
        assert_eq!(h.start_line_index(), Some(3));

        h.resolve(&mut doc, 2);
        assert_eq!(h.start_line_index(), Some(0));
    }

    #[test]
    fn resolve_unsets_when_no_heading_found() {
        let mut doc = Document::from_string("a\nb\n# C\nd\n".into());
        let mut h = Heading::new(Some(opts("^# ", 1)), &size(10, 8), 0, 0);

        h.resolve(&mut doc, 2);
        assert_eq!(h.start_line_index(), Some(2));

        h.resolve(&mut doc, 1);
        assert!(h.start_line_index().is_none());
        assert!(h.rows().is_empty());
    }

    #[test]
    fn resolve_if_found_keeps_current_when_none_in_range() {
        let mut doc = Document::from_string("# A\nx\ny\n".into());
        let mut h = Heading::new(Some(opts("^# ", 1)), &size(10, 8), 0, 0);

        h.resolve(&mut doc, 0);
        assert_eq!(h.start_line_index(), Some(0));

        h.resolve_if_found(&mut doc, 1..3);
        assert_eq!(h.start_line_index(), Some(0));
    }

    #[test]
    fn resolve_if_found_replaces_when_match_in_range() {
        let mut doc = Document::from_string("# A\nx\n# B\ny\n".into());
        let mut h = Heading::new(Some(opts("^# ", 1)), &size(10, 8), 0, 0);

        h.resolve(&mut doc, 0);
        assert_eq!(h.start_line_index(), Some(0));

        h.resolve_if_found(&mut doc, 1..3);
        assert_eq!(h.start_line_index(), Some(2));
    }

    #[test]
    fn push_up_offsets_visible_rows() {
        let mut doc = Document::from_string("# A\nsub\nfoo\nbar\n".into());
        let mut h = Heading::new(Some(opts("^# ", 2)), &size(10, 8), 0, 0);
        h.resolve(&mut doc, 1);
        assert_eq!(h.full_height(), 2);
        assert_eq!(h.height(), 2);

        h.push_up(1);
        assert_eq!(h.full_height(), 2);
        assert_eq!(h.height(), 1);
    }

    #[test]
    fn contains_returns_true_within_heading_lines() {
        let mut doc = Document::from_string("# A\nsub\nfoo\n".into());
        let mut h = Heading::new(Some(opts("^# ", 2)), &size(10, 8), 0, 0);
        h.resolve(&mut doc, 1);

        assert!(h.contains(0));
        assert!(h.contains(1));
        assert!(!h.contains(2));
    }

    #[test]
    fn min_line_index_excludes_global_header_area() {
        let mut doc = Document::from_string("# A\n# B\n# C\nx\ny\n".into());
        // The header covers 2 lines, so lines 0 and 1 never become headings.
        let mut h = Heading::new(Some(opts("^# ", 1)), &size(10, 10), 2, 2);

        h.resolve(&mut doc, 4);
        assert_eq!(h.start_line_index(), Some(2));
    }

    #[test]
    fn min_line_index_ignores_wrap_rows_of_the_global_header() {
        let mut doc = Document::from_string("# A\n# B\nx\ny\n".into());
        // The header covers 1 line but wraps into 2 rows, so line 1 is still a heading.
        let mut h = Heading::new(Some(opts("^# ", 1)), &size(10, 10), 2, 1);

        h.resolve(&mut doc, 3);
        assert_eq!(h.start_line_index(), Some(1));
    }

    #[test]
    fn unreachable_capped_header_still_uses_header_line_count() {
        let mut doc = Document::from_string("# A\n# B\n# C\nx\n".into());
        // height (2) < num_lines (3) only to pin the arithmetic, not to model a capped header:
        // that would force max_heading_height to 0, which find_heading short-circuits on first.
        let mut h = Heading::new(Some(opts("^# ", 1)), &size(10, 10), 2, 3);

        h.resolve(&mut doc, 3);
        assert!(h.start_line_index().is_none());
    }

    #[test]
    fn resize_rebuilds_rows_at_new_width() {
        let mut doc = Document::from_string("# Long header line\nsub\nfoo\n".into());
        let mut h = Heading::new(Some(opts("^# ", 1)), &size(80, 5), 0, 0);
        h.resolve(&mut doc, 0);
        assert_eq!(h.rows().len(), 1);

        h.resize(&mut doc, &size(5, 5), 0, 0);
        // "# Long header line" wraps into multiple rows at width 5.
        assert!(h.rows().len() > 1);
    }
}
