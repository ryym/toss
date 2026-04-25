use crate::{
    document::Document,
    line::Row,
    pager::{ViewportSize, rows},
};

/// Manages the global header. A fixed number of lines from the start of [`Document`]
/// are used as the header and are always shown at the top of the page.
/// The header content does not change during pagination, except when width changes due to resize.
#[derive(Debug)]
pub(super) struct GlobalHeader {
    num_lines: usize,
    rows: Vec<Row>,
}

impl GlobalHeader {
    pub fn new(doc: &mut Document, size: &ViewportSize, num_lines: usize) -> Self {
        let rows = build_rows(doc, size, num_lines);
        Self { num_lines, rows }
    }

    pub fn rows(&self) -> &[Row] {
        &self.rows
    }

    pub fn resize(&mut self, doc: &mut Document, size: &ViewportSize) {
        self.rows = build_rows(doc, size, self.num_lines);
    }

    pub fn height(&self) -> usize {
        self.rows.len()
    }

    pub fn contains(&self, line_index: usize) -> bool {
        line_index < self.num_lines
    }
}

fn build_rows(doc: &mut Document, size: &ViewportSize, num_lines: usize) -> Vec<Row> {
    // Reserve at least one non-header row so the header does not cover the entire viewport.
    let max_height = size.height() - 1;
    rows::from_lines(doc, size.width(), 0..num_lines, max_height)
}
