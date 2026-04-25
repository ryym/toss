use std::ops::Range;

use crate::{
    document::Document,
    line::Row,
    options::SectionOptions,
    pager::{ViewportSize, rows},
};

/// Manages the section header. Unlike the global header which always shows fixed lines,
/// a line matching the given pattern dynamically becomes the header and is shown pinned
/// below the global header. The behavior is similar to CSS `position: sticky`;
/// the displayed header changes as the user scrolls.
/// The nearest line matching the pattern among the lines above the first line of the viewport
/// becomes the start line of the section header.
#[derive(Debug)]
pub(super) struct SectionHeader {
    config: SectionConfig,
    options: Option<SectionOptions>,
    current: Option<HeaderState>,
}

impl SectionHeader {
    pub fn new(
        options: Option<SectionOptions>,
        size: &ViewportSize,
        global_header_height: usize,
    ) -> Self {
        Self {
            config: SectionConfig::new(size, global_header_height),
            options,
            current: None,
        }
    }

    /// Determine if the line at `line_index` is the start line of a section header.
    ///
    /// Example: with `toss --section '^#' --section-header 2`
    /// ```text
    /// # title     => is_header: true
    /// sub title   => is_header: false (it's a part of the header but not a start line)
    /// other line  => is_header: false
    /// ```
    ///
    /// Even when a line matches the section header pattern,
    /// if there is another line that also matches the pattern within the `--section-header` lines,
    /// the first line is NOT treated as a section header.
    ///
    /// Example: with `toss --section '^#' --section-header 2`
    /// ```text
    /// # title 1    => Not a header as there is `## title 2`
    /// ## title 2   => Not a header as there is `### title 3`
    /// ### title 3  => A header and is_header is true
    /// sentence 1   => A part of the header but not a start line (is_header is false)
    /// sentence 2   => Not a header
    /// ```
    pub fn is_header(&self, doc: &mut Document, line_index: usize) -> bool {
        match &self.options {
            Some(options) => is_header(doc, line_index, options),
            None => false,
        }
    }

    pub fn start_line_index(&self) -> Option<usize> {
        self.current.as_ref().map(|h| h.line_range.start)
    }

    /// The section rows visible in the page.
    pub fn rows(&self) -> &[Row] {
        match &self.current {
            None => &[],
            Some(h) => &h.rows[h.offset..],
        }
    }

    /// The section header height visible in the page.
    pub fn height(&self) -> usize {
        self.rows().len()
    }

    /// The section header height without accounting for the offset set by [`Self::push_up`].
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

    pub fn resize(&mut self, doc: &mut Document, size: &ViewportSize, global_header_height: usize) {
        self.config = SectionConfig::new(size, global_header_height);
        if let Some(h) = &mut self.current {
            h.rows = rows::from_lines(
                doc,
                self.config.width,
                h.line_range.clone(),
                self.config.max_header_height,
            );
        }
    }

    /// Shift the section header by `num_rows` upward and hide the top portion.
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

    /// Find and set the section header nearest to the given `line_index`.
    /// If the line of `line_index` itself is a part of the header, that header is set.
    /// If no section header is found above `line_index`, the current header is unset.
    pub fn resolve(&mut self, doc: &mut Document, line_index: usize) {
        self.current = self.find_header(doc, self.config.min_line_index..(line_index + 1));
    }

    /// Same as [`Self::resolve`] but keep the current section header when no header is found.
    pub fn resolve_if_found(&mut self, doc: &mut Document, line_index_range: Range<usize>) {
        if let Some(header) = self.find_header(doc, line_index_range) {
            self.current = Some(header);
        }
    }

    fn find_header(&self, doc: &mut Document, range: Range<usize>) -> Option<HeaderState> {
        let options = self.options.as_ref()?;
        if self.config.max_header_height == 0 {
            return None;
        }

        // Search for the section header nearest to `range.end` within the range.
        let mut nearest = None;
        if range.end > range.start {
            for i in (range.start..range.end).rev() {
                if is_header(doc, i, options) {
                    nearest = Some(i);
                    break;
                }
            }
        }
        let nearest = nearest?;

        let height = options.header_lines.min(self.config.max_header_height);
        let line_range = nearest..(nearest + height);
        let rows = rows::from_lines(
            doc,
            self.config.width,
            line_range.clone(),
            self.config.max_header_height,
        );
        Some(HeaderState {
            rows,
            line_range,
            offset: 0,
        })
    }
}

/// See [`SectionHeader::is_header`].
fn is_header(doc: &mut Document, line_index: usize, options: &SectionOptions) -> bool {
    match doc.line(line_index) {
        Some(line) if line.has_match(&options.pattern) => {}
        _ => return false,
    }
    // A line is treated as a header if it matches the pattern and no other line within the
    // following `header_lines` lines also matches the pattern.
    for i in 1..options.header_lines {
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
struct SectionConfig {
    /// The minimum line index that can be a section header.
    /// Lines below this index are never treated as section headers, regardless of pattern matching.
    min_line_index: usize,
    max_header_height: usize,
    width: usize,
}

impl SectionConfig {
    fn new(size: &ViewportSize, global_header_height: usize) -> Self {
        // Reserve at least one non-header row so the header does not cover the entire viewport.
        let max_header_height = size
            .height()
            .saturating_sub(global_header_height)
            .saturating_sub(1);
        Self {
            min_line_index: global_header_height,
            max_header_height,
            width: size.width(),
        }
    }
}

/// State representing a section header.
#[derive(Debug)]
struct HeaderState {
    line_range: Range<usize>,
    rows: Vec<Row>,
    /// Display `rows` starting from index `offset` in the viewport.
    /// Used when the section header transitions during scrolling.
    /// For example, when scrolling down and approaching the next section header,
    /// the current header gradually increases its `offset` to disappear upward,
    /// and is replaced row by row by the new header.
    offset: usize,
}
