use std::ops::Range;

use crate::{
    document::Document,
    line::Row,
    options::{HeadingOptions, Options},
    pager::{
        ViewportSize,
        rows::{self, DocPos},
    },
};

/// A row position in the document: `(line_index, wrap_index)`.
pub(super) type RowPos = (usize, usize);

/// Everything needed to compose a [`Frame`] except the anchor.
/// This is the static part of the page: only a resize changes what it describes.
#[derive(Debug)]
pub(super) struct Layout {
    size: ViewportSize,
    /// Number of leading document lines pinned as the global header (`--header`).
    header_lines: usize,
    /// The heading configuration (`--heading`), along with the memo it accumulates as
    /// pages are composed. `None` disables the sticky heading entirely.
    heading: Option<Headings>,
}

impl Layout {
    pub fn new(options: Options, size: ViewportSize) -> Self {
        Self {
            size,
            header_lines: options.header,
            heading: options.heading.map(Headings::new),
        }
    }

    /// Change the viewport size. Nothing else the layout describes depends on it.
    pub fn resize(&mut self, size: ViewportSize) {
        // Keep the heading memo: which lines start a heading does not depend on the size.
        self.size = size;
    }

    #[inline]
    pub fn size(&self) -> &ViewportSize {
        &self.size
    }

    /// Whether `line_index` falls in the configured header range.
    /// This is the configured extent, not the number of rows the header renders as.
    pub fn is_header_line(&self, line_index: usize) -> bool {
        line_index < self.header_lines
    }

    /// Rows the global header may occupy, always leaving at least one row for content.
    fn max_header_height(&self) -> usize {
        self.size.height().saturating_sub(1)
    }

    /// Rows the sticky heading may occupy given the rows the header took,
    /// always leaving at least one row for content.
    fn max_heading_height(&self, header: &[Row]) -> usize {
        self.size
            .height()
            .saturating_sub(header.len())
            .saturating_sub(1)
    }
}

/// The heading pattern plus a memo of where the heading start lines are.
///
/// Composing a page needs the nearest heading start at or above a given line, which is a
/// backward scan over the document. Without a memo that scan is proportional to the
/// document length on every frame, and a document with no heading in it pays the worst
/// case every time.
///
/// The memo never needs invalidating. Lines are immutable and the document only grows, so
/// an answer stays true once computed. The one exception is the tail: [`is_heading_start`]
/// also looks at the following `--heading-lines - 1` lines, so a line that is a heading
/// start only because those lines have not arrived yet may stop being one later. Those
/// lines are answered by scanning but never recorded.
#[derive(Debug)]
struct Headings {
    options: HeadingOptions,
    /// Confirmed heading start lines, ascending.
    starts: Vec<usize>,
    /// Lines already tested, as one contiguous range. `starts` is complete within it, so
    /// this is exactly where the memo can also answer "no heading start here".
    tested: Range<usize>,
}

impl Headings {
    fn new(options: HeadingOptions) -> Self {
        Self {
            options,
            starts: Vec::new(),
            tested: 0..0,
        }
    }

    /// The nearest heading start in `lo..=at`, touching the document only for the lines
    /// the memo cannot answer.
    fn start_at_or_above(&mut self, doc: &mut Document, lo: usize, at: usize) -> Option<usize> {
        if at < lo {
            return None;
        }
        // Answers for lines this close to the end of a growing document are not final yet.
        let final_end = if doc.is_complete() {
            usize::MAX
        } else {
            doc.line_count()
                .saturating_sub(self.options.num_lines.saturating_sub(1))
        };

        let mut line = at;
        let found = loop {
            if self.tested.contains(&line) {
                // The memo covers this line down to `tested.start`.
                let from = self.tested.start.max(lo);
                if let Some(start) = self.recorded_start_in(from..(line + 1)) {
                    break Some(start);
                }
                if self.tested.start <= lo {
                    break None;
                }
                // The memo ran out above `lo`: keep scanning below it.
                line = self.tested.start - 1;
                continue;
            }
            if is_heading_start(doc, line, &self.options) {
                if line < final_end {
                    self.record_start(line);
                }
                break Some(line);
            }
            if line == lo {
                break None;
            }
            line -= 1;
        };
        // Everything from `line` up to `at` has now been tested, one way or another.
        self.mark_tested(line..(at + 1).min(final_end));
        found
    }

    /// The greatest recorded start within `range`.
    fn recorded_start_in(&self, range: Range<usize>) -> Option<usize> {
        let end = self.starts.partition_point(|&start| start < range.end);
        let start = *self.starts[..end].last()?;
        (start >= range.start).then_some(start)
    }

    fn record_start(&mut self, line_index: usize) {
        if let Err(i) = self.starts.binary_search(&line_index) {
            self.starts.insert(i, line_index);
        }
    }

    fn mark_tested(&mut self, range: Range<usize>) {
        if range.start >= range.end {
            return;
        }
        let overlaps = range.start <= self.tested.end && self.tested.start <= range.end;
        self.tested = if self.tested.is_empty() || !overlaps {
            // A single range is all the memo tracks, so a stretch disconnected from what is
            // known replaces it. The recorded starts stay valid either way.
            range
        } else {
            self.tested.start.min(range.start)..self.tested.end.max(range.end)
        };
    }
}

/// A composed page: the whole visible state, derived from a [`Layout`] and an anchor.
///
/// [`Self::rows`] holds one document row per viewport row starting at the anchor.
/// The first [`Self::overlay_height`] of them are covered by the sticky rows
/// ([`Self::header`] and [`Self::heading`]) and never rendered; the rest is the content.
#[derive(Debug)]
pub(super) struct Frame {
    rows: Vec<Row>,
    header: Vec<Row>,
    /// Heading rows as displayed, i.e. already trimmed by the push-up offset.
    heading: Vec<Row>,
}

impl Frame {
    /// The anchor this frame was composed from, after clamping.
    pub fn anchor(&self) -> RowPos {
        self.rows
            .first()
            .map_or((0, 0), |r| (r.line_index(), r.wrap_index()))
    }

    /// All document rows the viewport spans, including the ones the overlay covers.
    pub fn rows(&self) -> &[Row] {
        &self.rows
    }

    pub fn header(&self) -> &[Row] {
        &self.header
    }

    pub fn heading(&self) -> &[Row] {
        &self.heading
    }

    /// Number of document rows hidden behind the sticky area.
    pub fn overlay_height(&self) -> usize {
        self.header.len() + self.heading.len()
    }

    /// The document rows actually visible below the sticky area.
    pub fn content(&self) -> &[Row] {
        let overlay = self.overlay_height().min(self.rows.len());
        &self.rows[overlay..]
    }

    /// The visible rows that form a contiguous range of the document, in reading order.
    /// The sticky rows are included only while they sit directly above the content in the
    /// document; otherwise only the content is returned.
    ///
    /// For example, if the heading shows lines 3-5 and the content shows lines 6-30, the
    /// rows for lines 3-30 are returned, plus the global header when it covers lines 1-2.
    /// If the content instead starts at line 7, the sticky rows are dropped.
    pub fn contiguous_rows(&self) -> Vec<Row> {
        let mut rows: Vec<Row> = Vec::new();
        let content = self.content();
        let top_line = match (content.first(), self.heading.first()) {
            (Some(row), _) => row.line_index(),
            // Nothing is visible below the overlay: the heading itself is the whole page.
            (None, Some(row)) => row.line_index(),
            // Not even a heading: the header is the whole page, and trivially contiguous.
            (None, None) => return self.header.clone(),
        };

        let heading_adjacent = match self.heading.last() {
            Some(last) => last.line_index() + 1 >= top_line,
            None => true,
        };
        let heading_top = self.heading.first().map_or(top_line, |r| r.line_index());
        // Measure the header against the heading only while the heading itself joins the
        // content; a detached heading is dropped, so the header must reach the content.
        let above_line = if heading_adjacent {
            heading_top
        } else {
            top_line
        };
        let header_adjacent = match self.header.last() {
            Some(last) => last.line_index() + 1 >= above_line,
            None => true,
        };

        if header_adjacent && heading_adjacent {
            rows.extend_from_slice(&self.header);
        }
        if heading_adjacent {
            rows.extend_from_slice(&self.heading);
        }
        rows.extend_from_slice(content);
        rows
    }
}

/// Build the whole page for `anchor`.
///
/// This is the single place where the header, the sticky heading and the content are
/// decided; every page operation reduces to picking an anchor and calling this.
pub(super) fn compose(doc: &mut Document, layout: &mut Layout, anchor: RowPos) -> Frame {
    let width = layout.size.width();
    let header = rows::from_lines(
        doc,
        width,
        0..layout.header_lines,
        layout.max_header_height(),
    );
    let rows = fill_from(doc, layout, anchor);

    // The heading sticks to the first row below the global header. Resolving from that row
    // rather than from the first *visible* row is what makes the push-up transition work:
    // a heading that has scrolled into the covered band pushes the current one out row by
    // row, and takes over exactly when it reaches the top of the band.
    let block = rows
        .get(header.len())
        .map(|row| row.line_index())
        .and_then(|line| resolve_heading(doc, layout, &header, line));

    let heading = match block {
        None => Vec::new(),
        Some(block) => {
            let push_up = push_up_offset(doc, layout, &rows, &header, &block);
            block.rows[push_up..].to_vec()
        }
    };

    Frame {
        rows,
        header,
        heading,
    }
}

/// List the viewport rows starting at `anchor`, pulling the anchor back toward the start of
/// the document when there are not enough rows left to fill the page. Keeping the page full
/// is what makes growing the terminal near the end of the document behave like `less`.
fn fill_from(doc: &mut Document, layout: &Layout, anchor: RowPos) -> Vec<Row> {
    let width = layout.size.width();
    let height = layout.size.height();
    let rows = rows::list_forward(doc, width, anchor, height);
    if rows.len() >= height {
        return rows;
    }

    let missing = height - rows.len();
    let earlier = match rows.first() {
        Some(first) => rows::list_backward(doc, width, DocPos::Before(first), missing),
        // The anchor is past the end of the document: fall back to its last page.
        None => return rows::list_backward(doc, width, DocPos::End, height),
    };
    if earlier.is_empty() {
        return rows;
    }
    let mut filled = earlier;
    filled.extend(rows);
    filled
}

/// A heading block resolved for a given position, before the push-up is applied.
struct HeadingBlock {
    start_line: usize,
    rows: Vec<Row>,
}

/// Find the heading the line at `at_line` belongs to: the nearest line at or above it that
/// starts a heading. Lines covered by the global header are never candidates, since the
/// header already shows them.
fn resolve_heading(
    doc: &mut Document,
    layout: &mut Layout,
    header: &[Row],
    at_line: usize,
) -> Option<HeadingBlock> {
    let max_height = layout.max_heading_height(header);
    if max_height == 0 || at_line < layout.header_lines {
        return None;
    }

    let header_lines = layout.header_lines;
    let width = layout.size.width();
    let headings = layout.heading.as_mut()?;
    let start_line = headings.start_at_or_above(doc, header_lines, at_line)?;
    let line_range = start_line..(start_line + headings.options.num_lines);
    let rows = rows::from_lines(doc, width, line_range, max_height);
    if rows.is_empty() {
        return None;
    }
    Some(HeadingBlock { start_line, rows })
}

/// How many rows the current heading must be shifted up by.
///
/// When another heading has scrolled into the band the overlay covers, a section
/// transition is in progress: the current heading gives up one row for every row the new
/// one has advanced, so the new heading stays visible as it rises into the sticky area.
fn push_up_offset(
    doc: &mut Document,
    layout: &Layout,
    rows: &[Row],
    header: &[Row],
    block: &HeadingBlock,
) -> usize {
    let Some(options) = layout.heading.as_ref().map(|h| &h.options) else {
        return 0;
    };
    let overlay = header.len() + block.rows.len();
    let mut next_section_start = overlay;
    for (i, row) in rows.iter().enumerate().take(overlay).skip(header.len()) {
        if row.wrap_index() != 0
            || row.line_index() == block.start_line
            || layout.is_header_line(row.line_index())
        {
            continue;
        }
        if is_heading_start(doc, row.line_index(), options) {
            next_section_start = i;
            break;
        }
    }
    overlay.saturating_sub(next_section_start)
}

/// Whether the line at `line_index` starts a heading block.
///
/// Example: with `toss --heading '^#' --heading-lines 2`
/// ```text
/// # title     => is_heading_start: true
/// sub title   => is_heading_start: false (not a start line)
/// other line  => is_heading_start: false
/// ```
///
/// Even when a line matches the heading pattern, if another line within the following
/// `--heading-lines` lines also matches, the earlier line is NOT a heading start.
///
/// Example: with `toss --heading '^#' --heading-lines 2`
/// ```text
/// # title 1    => Not a heading as there is `## title 2`
/// ## title 2   => Not a heading as there is `### title 3`
/// ### title 3  => A heading and is_heading_start is true
/// sentence 1   => A part of the heading but not a start line
/// sentence 2   => Not a heading
/// ```
fn is_heading_start(doc: &mut Document, line_index: usize, options: &HeadingOptions) -> bool {
    match doc.line(line_index) {
        Some(line) if line.has_match(&options.pattern) => {}
        _ => return false,
    }
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

/// Where the heading would sit for a page showing `at_line`.
pub(super) struct HeadingPlacement {
    /// Document lines the heading block displays.
    pub lines: Range<usize>,
    /// Rows the heading block occupies on screen.
    pub height: usize,
}

/// Where the heading would sit on a page whose content reaches `at_line`, or `None` if no
/// heading would be pinned there.
pub(super) fn heading_placement(
    doc: &mut Document,
    layout: &mut Layout,
    header: &[Row],
    at_line: usize,
) -> Option<HeadingPlacement> {
    let block = resolve_heading(doc, layout, header, at_line)?;
    let end = block.rows[block.rows.len() - 1].line_index() + 1;
    Some(HeadingPlacement {
        lines: block.start_line..end,
        height: block.rows.len(),
    })
}

/// The anchor that shows the last page of the document.
pub(super) fn end_anchor(doc: &mut Document, layout: &Layout) -> RowPos {
    let last_page =
        rows::list_backward(doc, layout.size.width(), DocPos::End, layout.size.height());
    last_page
        .first()
        .map_or((0, 0), |r| (r.line_index(), r.wrap_index()))
}

/// The anchor `count` rows after `from`, clamped to the last row of the document.
pub(super) fn anchor_forward(
    doc: &mut Document,
    layout: &Layout,
    from: RowPos,
    count: usize,
) -> RowPos {
    let ahead = rows::list_forward(doc, layout.size.width(), from, count + 1);
    ahead
        .last()
        .map_or(from, |r| (r.line_index(), r.wrap_index()))
}

/// The anchor `count` rows before `from`, clamped to the first row of the document.
/// Putting a target line `count` rows below the top of the page is the same operation:
/// pass the target as `from`.
pub(super) fn anchor_backward(
    doc: &mut Document,
    layout: &Layout,
    from: RowPos,
    count: usize,
) -> RowPos {
    if count == 0 {
        return from;
    }
    let width = layout.size.width();
    let (line_index, wrap_index) = from;
    let from_row = {
        let Some(line) = doc.line(line_index) else {
            return from;
        };
        match line.wrap(width).into_iter().nth(wrap_index) {
            Some(row) => row,
            None => return from,
        }
    };
    let earlier = rows::list_backward(doc, width, DocPos::Before(&from_row), count);
    earlier
        .first()
        .map_or(from, |r| (r.line_index(), r.wrap_index()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::Document;
    use crate::document::StreamMsg;
    use crate::line::Line;
    use regex::Regex;
    use std::sync::mpsc;

    fn size(width: usize, height: usize) -> ViewportSize {
        ViewportSize { width, height }
    }

    fn layout(header: usize, heading: Option<(&str, usize)>, size: ViewportSize) -> Layout {
        let options = Options {
            header,
            heading: heading.map(|(pattern, num_lines)| HeadingOptions {
                pattern: Regex::new(pattern).unwrap(),
                num_lines,
            }),
            quit_if_one_screen: false,
        };
        Layout::new(options, size)
    }

    fn pos(rows: &[Row]) -> Vec<(usize, usize)> {
        rows.iter()
            .map(|r| (r.line_index(), r.wrap_index()))
            .collect()
    }

    fn lines(rows: &[Row]) -> Vec<usize> {
        rows.iter().map(|r| r.line_index()).collect()
    }

    fn doc_lines(n: usize) -> Document {
        let s = (0..n)
            .map(|i| format!("line{i}"))
            .collect::<Vec<_>>()
            .join("\n");
        Document::from_string(s)
    }

    fn heading_options(pattern: &str, num_lines: usize) -> HeadingOptions {
        HeadingOptions {
            pattern: Regex::new(pattern).unwrap(),
            num_lines,
        }
    }

    /// 20 lines with a heading at 0, 5 and 12.
    fn doc_with_headings() -> Document {
        let mut lines: Vec<String> = (0..20).map(|i| format!("line{i}")).collect();
        for i in [0, 5, 12] {
            lines[i] = format!("# h{i}");
        }
        Document::from_string(lines.join("\n"))
    }

    #[test]
    fn the_heading_memo_answers_like_a_full_scan() {
        let mut doc = doc_with_headings();
        let options = heading_options("^# ", 1);
        let mut headings = Headings::new(options.clone());

        // Query out of order, so the memo also meets lines far below what it has scanned.
        for at in [19, 18, 3, 4, 13, 12, 11, 6, 0, 19] {
            let expected = (0..=at)
                .rev()
                .find(|&i| is_heading_start(&mut doc, i, &options));
            assert_eq!(
                headings.start_at_or_above(&mut doc, 0, at),
                expected,
                "at line {at}"
            );
        }
    }

    #[test]
    fn the_heading_memo_never_reaches_below_the_lower_bound() {
        let mut doc = doc_with_headings();
        let mut headings = Headings::new(heading_options("^# ", 1));
        assert_eq!(headings.start_at_or_above(&mut doc, 0, 19), Some(12));
        assert_eq!(headings.start_at_or_above(&mut doc, 6, 11), None);
        assert_eq!(headings.start_at_or_above(&mut doc, 6, 19), Some(12));
        assert_eq!(headings.start_at_or_above(&mut doc, 1, 4), None);
    }

    #[test]
    fn a_heading_start_at_the_end_of_a_growing_document_is_not_memoized() {
        let (tx, rx) = mpsc::channel();
        let mut doc = Document::from_channel(rx);
        tx.send(StreamMsg::Line(Line::new(0, "# a".into())))
            .unwrap();
        doc.pump();

        // With --heading-lines 2, "# a" is a heading start only while the line that would
        // follow it is still unknown.
        let mut headings = Headings::new(heading_options("^# ", 2));
        assert_eq!(headings.start_at_or_above(&mut doc, 0, 0), Some(0));

        tx.send(StreamMsg::Line(Line::new(1, "# b".into())))
            .unwrap();
        doc.pump();
        assert_eq!(headings.start_at_or_above(&mut doc, 0, 0), None);
    }

    #[test]
    fn composes_from_the_top_of_the_document() {
        let mut doc = doc_lines(10);
        let mut layout = layout(0, None, size(10, 4));
        let frame = compose(&mut doc, &mut layout, (0, 0));
        assert!(frame.header().is_empty());
        assert!(frame.heading().is_empty());
        assert_eq!(pos(frame.content()), vec![(0, 0), (1, 0), (2, 0), (3, 0)]);
    }

    #[test]
    fn short_document_leaves_the_page_partially_filled() {
        let mut doc = doc_lines(2);
        let mut layout = layout(0, None, size(10, 5));
        let frame = compose(&mut doc, &mut layout, (0, 0));
        assert_eq!(lines(frame.content()), vec![0, 1]);
    }

    #[test]
    fn header_covers_the_rows_it_duplicates() {
        let mut doc = doc_lines(10);
        let mut layout = layout(2, None, size(10, 5));
        let frame = compose(&mut doc, &mut layout, (0, 0));
        // The header shows lines 0-1, which also occupy the first two viewport rows.
        assert_eq!(lines(frame.header()), vec![0, 1]);
        assert_eq!(lines(frame.rows()), vec![0, 1, 2, 3, 4]);
        assert_eq!(lines(frame.content()), vec![2, 3, 4]);
    }

    #[test]
    fn header_is_capped_to_leave_room_for_content() {
        let mut doc = doc_lines(10);
        // Viewport height 5 leaves at most 4 rows for a header.
        let mut layout = layout(5, None, size(10, 5));
        let frame = compose(&mut doc, &mut layout, (0, 0));
        assert_eq!(lines(frame.header()), vec![0, 1, 2, 3]);
    }

    #[test]
    fn anchor_is_pulled_back_to_keep_the_page_full() {
        let mut doc = doc_lines(6);
        let mut layout = layout(0, None, size(10, 4));
        // Only two rows are left below line 4, so the anchor moves back to line 2.
        let frame = compose(&mut doc, &mut layout, (4, 0));
        assert_eq!(frame.anchor(), (2, 0));
        assert_eq!(lines(frame.content()), vec![2, 3, 4, 5]);
    }

    #[test]
    fn anchor_past_the_end_falls_back_to_the_last_page() {
        let mut doc = doc_lines(6);
        let mut layout = layout(0, None, size(10, 4));
        let frame = compose(&mut doc, &mut layout, (99, 0));
        assert_eq!(lines(frame.content()), vec![2, 3, 4, 5]);
    }

    #[test]
    fn heading_sticks_to_the_row_below_the_header() {
        let content = "\
# A
a1
a2
# B
b1
b2
b3
";
        let mut doc = Document::from_string(content.into());
        let mut layout = layout(0, Some(("^# ", 1)), size(10, 4));
        let frame = compose(&mut doc, &mut layout, (2, 0));
        // Line 2 belongs to section A, so "# A" is pinned and covers line 2.
        assert_eq!(lines(frame.heading()), vec![0]);
        assert_eq!(lines(frame.content()), vec![3, 4, 5]);
    }

    #[test]
    fn heading_is_pushed_up_by_the_next_section() {
        let content = "\
# A
a1
a2
# B
b1
b2
b3
b4
b5
";
        let mut doc = Document::from_string(content.into());
        // --heading-lines 2 so the heading occupies two rows and can be pushed up by one.
        let mut layout = layout(0, Some(("^# ", 2)), size(10, 6));
        // Anchor at line 2: the covered band is lines 2-3 and line 3 starts section B.
        let frame = compose(&mut doc, &mut layout, (2, 0));
        // "# A" is pushed up by one row, leaving only its second line pinned.
        assert_eq!(lines(frame.heading()), vec![1]);
        // The row freed by the push-up reveals "# B" as content.
        assert_eq!(lines(frame.content()), vec![3, 4, 5, 6, 7]);
    }

    #[test]
    fn heading_never_comes_from_the_global_header() {
        let content = "\
# A
# B
b1
b2
b3
";
        let mut doc = Document::from_string(content.into());
        // Lines 0-1 are the global header, so neither may become the heading.
        let mut layout = layout(2, Some(("^# ", 1)), size(10, 5));
        let frame = compose(&mut doc, &mut layout, (2, 0));
        assert!(frame.heading().is_empty());
        assert_eq!(lines(frame.header()), vec![0, 1]);
    }

    #[test]
    fn no_heading_when_none_precedes_the_anchor() {
        let content = "\
a0
a1
# B
b1
";
        let mut doc = Document::from_string(content.into());
        let mut layout = layout(0, Some(("^# ", 1)), size(10, 3));
        let frame = compose(&mut doc, &mut layout, (0, 0));
        assert!(frame.heading().is_empty());
    }

    #[test]
    fn contiguous_rows_join_adjacent_sticky_and_content() {
        let content = "\
h0
# A
a1
a2
a3
";
        let mut doc = Document::from_string(content.into());
        let mut layout = layout(1, Some(("^# ", 1)), size(10, 4));
        let frame = compose(&mut doc, &mut layout, (0, 0));
        // The header displays line 0 and covers it, the heading does the same for line 1,
        // so the whole page reads as one contiguous range.
        assert_eq!(lines(frame.header()), vec![0]);
        assert_eq!(lines(frame.heading()), vec![1]);
        assert_eq!(lines(&frame.contiguous_rows()), vec![0, 1, 2, 3]);
    }

    #[test]
    fn contiguous_rows_drop_sticky_rows_detached_from_the_content() {
        let content = "\
h0
# A
a1
a2
a3
a4
a5
";
        let mut doc = Document::from_string(content.into());
        let mut layout = layout(1, Some(("^# ", 1)), size(10, 4));
        let frame = compose(&mut doc, &mut layout, (3, 0));
        // The heading (line 1) is far above the content (lines 5-6), so only content remains.
        assert_eq!(lines(frame.heading()), vec![1]);
        assert_eq!(lines(&frame.contiguous_rows()), vec![5, 6]);
    }

    #[test]
    fn wrapped_lines_occupy_several_rows() {
        let mut doc = Document::from_string("abcde\nf\ng\n".into());
        let mut layout = layout(0, None, size(2, 4));
        let frame = compose(&mut doc, &mut layout, (0, 0));
        assert_eq!(pos(frame.content()), vec![(0, 0), (0, 1), (0, 2), (1, 0)]);
    }
}
