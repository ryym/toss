use crate::{
    document::Document,
    line::Row,
    pager::{
        ViewportSize,
        rows::{self, DocPos},
    },
};

/// Manages the rows of [`Document`] lines displayed in the page.
/// See [`crate::pager::Pager`] for details.
#[derive(Debug)]
pub(super) struct Viewport {
    size: ViewportSize,
    rows: Vec<Row>,
}

impl Viewport {
    pub fn new(doc: &mut Document, size: ViewportSize) -> Self {
        let rows = rows::list_forward(doc, size.width(), (0, 0), size.height());
        Self { size, rows }
    }

    #[inline]
    pub fn size(&self) -> &ViewportSize {
        &self.size
    }

    #[inline]
    pub fn rows(&self) -> &[Row] {
        &self.rows
    }

    /// Search for the given line and wrap within the viewport's rows,
    /// and return its index in rows if found.
    pub fn row_index(&self, line_index: usize, wrap_index: usize) -> Option<usize> {
        self.rows.iter().enumerate().find_map(|(i, row)| {
            if row.line_index() == line_index && row.wrap_index() == wrap_index {
                Some(i)
            } else {
                None
            }
        })
    }

    /// Rebuild the rows from the top of the document.
    /// Used while the first screen is still filling in from streamed input.
    pub fn refill_from_top(&mut self, doc: &mut Document) {
        self.rows = rows::list_forward(doc, self.size.width(), (0, 0), self.size.height());
    }

    pub fn resize(&mut self, doc: &mut Document, size: ViewportSize) {
        self.rows = rows::list_forward(
            doc,
            size.width(),
            (self.rows[0].line_index(), self.rows[0].wrap_index()),
            size.height(),
        );
        self.size = size;
    }

    /// Remove the specified number of rows from the end and prepend the same number of new rows.
    pub fn scroll_up(&mut self, doc: &mut Document, n_rows: usize) -> usize {
        if n_rows == 0 || self.rows.is_empty() {
            return 0;
        }
        assert!(
            n_rows <= self.size.height,
            "[viewport] scroll up rows too big: {n_rows} > height {}",
            self.size.height
        );

        // Fetch the specified number of new rows.
        let first = self.rows[0].clone();
        let new_rows = rows::list_backward(doc, self.size.width, DocPos::Before(&first), n_rows);
        let scroll_rows = new_rows.len();
        log::debug!("scroll up {new_rows:?} {scroll_rows}");

        // Remove the same number of rows from the end and prepend the new rows.
        let len = self.rows.len();
        self.rows.truncate(len - new_rows.len());
        self.rows.splice(0..0, new_rows);

        scroll_rows
    }

    /// Remove the specified number of rows from the start and append the same number of new rows.
    pub fn scroll_down(&mut self, doc: &mut Document, n_rows: usize) -> usize {
        if n_rows == 0 || self.rows.is_empty() {
            return 0;
        }
        assert!(
            n_rows <= self.size.height,
            "[viewport] scroll down rows too big: {n_rows} > height {}",
            self.size.height
        );

        // Fetch the specified number of new rows.
        let last = self.rows.last().unwrap().clone();
        let new_rows = rows::list_forward(
            doc,
            self.size.width,
            (last.line_index(), last.wrap_index() + 1),
            n_rows,
        );
        let scroll_rows = new_rows.len();

        // Remove the same number of rows from the start and append the new rows.
        let remove_count = new_rows.len();
        self.rows.drain(0..remove_count);
        self.rows.extend(new_rows);

        scroll_rows
    }

    /// Jump to a specific line, rebuilding the rows from there.
    /// Returns the final index of the given line within the viewport rows.
    /// If the line is near the end of the file, it may end up positioned below `row_offset`.
    pub fn jump_to(&mut self, doc: &mut Document, line_index: usize, row_offset: usize) -> usize {
        let height = self.size.height;
        let rows_after_line = rows::list_forward(doc, self.size.width, (line_index, 0), height);

        let padding = height
            .saturating_sub(1)
            .min(row_offset.max(height.saturating_sub(rows_after_line.len())));
        let rows_before_line = rows::list_backward(
            doc,
            self.size.width,
            DocPos::Before(&rows_after_line[0]),
            padding,
        );

        let final_line_row_index = rows_before_line.len();
        let mut combined: Vec<Row> = rows_before_line;
        combined.extend(rows_after_line);
        combined.truncate(height);
        self.rows = combined;

        final_line_row_index
    }

    /// Jump to the end of the document so that the last line is at the bottom.
    pub fn jump_to_end(&mut self, doc: &mut Document) {
        // Fill up to the full height: while streaming, the viewport may not yet
        // be full, so using the current row count would under-fill the page.
        self.rows = rows::list_backward(doc, self.size.width, DocPos::End, self.size.height);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::Document;

    fn pos(rows: &[Row]) -> Vec<(usize, usize)> {
        rows.iter()
            .map(|r| (r.line_index(), r.wrap_index()))
            .collect()
    }

    fn size(width: usize, height: usize) -> ViewportSize {
        ViewportSize { width, height }
    }

    #[test]
    fn fills_from_top_on_construction() {
        let mut doc = Document::from_string("a\nb\nc\nd\ne\nf\n".into());
        let v = Viewport::new(&mut doc, size(10, 4));
        assert_eq!(pos(v.rows()), vec![(0, 0), (1, 0), (2, 0), (3, 0)]);
    }

    #[test]
    fn fills_partially_when_doc_is_short() {
        let mut doc = Document::from_string("a\nb\n".into());
        let v = Viewport::new(&mut doc, size(10, 5));
        assert_eq!(pos(v.rows()), vec![(0, 0), (1, 0)]);
    }

    #[test]
    fn row_index_finds_existing_row() {
        let mut doc = Document::from_string("a\nb\nc\n".into());
        let v = Viewport::new(&mut doc, size(10, 3));
        assert_eq!(v.row_index(1, 0), Some(1));
        assert_eq!(v.row_index(5, 0), None);
    }

    #[test]
    fn scroll_down_reveals_lower_rows() {
        let mut doc = Document::from_string("a\nb\nc\nd\ne\n".into());
        let mut v = Viewport::new(&mut doc, size(10, 3));
        let scrolled = v.scroll_down(&mut doc, 2);
        assert_eq!(scrolled, 2);
        assert_eq!(pos(v.rows()), vec![(2, 0), (3, 0), (4, 0)]);
    }

    #[test]
    fn scroll_down_clamps_at_doc_end() {
        let mut doc = Document::from_string("a\nb\nc\nd\n".into());
        let mut v = Viewport::new(&mut doc, size(10, 3));
        let scrolled = v.scroll_down(&mut doc, 3);
        assert_eq!(scrolled, 1);
        assert_eq!(pos(v.rows()), vec![(1, 0), (2, 0), (3, 0)]);
    }

    #[test]
    fn scroll_up_reveals_upper_rows() {
        let mut doc = Document::from_string("a\nb\nc\nd\ne\n".into());
        let mut v = Viewport::new(&mut doc, size(10, 3));
        v.scroll_down(&mut doc, 2);
        let scrolled = v.scroll_up(&mut doc, 1);
        assert_eq!(scrolled, 1);
        assert_eq!(pos(v.rows()), vec![(1, 0), (2, 0), (3, 0)]);
    }

    #[test]
    fn scroll_up_clamps_at_doc_start() {
        let mut doc = Document::from_string("a\nb\nc\n".into());
        let mut v = Viewport::new(&mut doc, size(10, 3));
        let scrolled = v.scroll_up(&mut doc, 2);
        assert_eq!(scrolled, 0);
        assert_eq!(pos(v.rows()), vec![(0, 0), (1, 0), (2, 0)]);
    }

    #[test]
    fn jump_to_places_line_below_offset() {
        let mut doc = Document::from_string("a\nb\nc\nd\ne\nf\ng\n".into());
        let mut v = Viewport::new(&mut doc, size(10, 4));
        let line_pos = v.jump_to(&mut doc, 4, 1);
        assert_eq!(line_pos, 1);
        assert_eq!(pos(v.rows()), vec![(3, 0), (4, 0), (5, 0), (6, 0)]);
    }

    #[test]
    fn jump_to_anchors_at_bottom_when_near_end() {
        let mut doc = Document::from_string("a\nb\nc\nd\ne\n".into());
        let mut v = Viewport::new(&mut doc, size(10, 4));
        let line_pos = v.jump_to(&mut doc, 4, 0);
        assert_eq!(line_pos, 3);
        assert_eq!(pos(v.rows()), vec![(1, 0), (2, 0), (3, 0), (4, 0)]);
    }

    #[test]
    fn jump_to_end_anchors_last_line_at_bottom() {
        let mut doc = Document::from_string("a\nb\nc\nd\ne\n".into());
        let mut v = Viewport::new(&mut doc, size(10, 3));
        v.jump_to_end(&mut doc);
        assert_eq!(pos(v.rows()), vec![(2, 0), (3, 0), (4, 0)]);
    }

    #[test]
    fn resize_keeps_top_row_anchored() {
        let mut doc = Document::from_string("a\nb\nc\nd\ne\nf\n".into());
        let mut v = Viewport::new(&mut doc, size(10, 3));
        v.scroll_down(&mut doc, 2);
        assert_eq!(v.rows()[0].line_index(), 2);

        v.resize(&mut doc, size(10, 4));
        assert_eq!(v.rows()[0].line_index(), 2);
        assert_eq!(v.rows().len(), 4);
    }
}
