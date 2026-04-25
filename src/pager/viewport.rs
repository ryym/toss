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

    pub fn resize(&mut self, doc: &mut Document, size: ViewportSize) {
        self.rows = rows::list_forward(
            doc,
            size.width(),
            (self.rows[0].line_index(), self.rows[0].wrap_index()),
            size.height(),
        );
        self.size = size;
    }

    // Remove the specified number of rows from the end and prepend the same number of new rows.
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

    // Remove the specified number of rows from the start and append the same number of new rows.
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

    // Jump to a specific line, rebuilding the rows from there.
    // Returns the final index of the given line within the viewport rows.
    // If the line is near the end of the file, it may end up positioned below `row_offset`.
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
        let count = self.rows.len();
        self.rows = rows::list_backward(doc, self.size.width, DocPos::End, count);
    }
}
