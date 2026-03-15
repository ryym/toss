use crate::document::Document;
use crate::viewport::ScreenRow;

/// Manages sticky header lines pinned at the top of the screen.
pub struct Header {
    /// Number of logical lines to pin from the top of the document.
    fixed_lines: usize,
}

impl Header {
    /// Create a new header with the given number of fixed lines.
    pub fn new(fixed_lines: usize) -> Self {
        Self { fixed_lines }
    }

    /// The minimum line index that the viewport may start from.
    /// Lines below this are reserved for the header display.
    pub fn min_top_line(&self) -> usize {
        self.fixed_lines
    }

    /// Resolve the header rows to display, accounting for line wrapping.
    pub fn resolve(&self, doc: &mut Document, width: usize) -> Vec<ScreenRow> {
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
