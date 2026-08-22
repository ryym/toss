use crate::{
    document::Document,
    line::Row,
    pager::{ViewportSize, rows},
};

/// Manages the global header. A fixed number of lines from the start of [`Document`]
/// are used as the header and are always shown at the top of the page.
/// The header content does not change during pagination, except when width changes due to resize.
#[derive(Debug)]
pub(super) struct Header {
    num_lines: usize,
    rows: Vec<Row>,
}

impl Header {
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

    /// The number of screen rows the header covers. Unlike [`Self::num_lines`], this counts
    /// rendered rows: larger than `num_lines` when header lines wrap, smaller when the header
    /// is capped to fit the viewport or the document has fewer lines than configured.
    pub fn height(&self) -> usize {
        self.rows.len()
    }

    /// The number of leading document lines configured as the header.
    /// This is a configured extent, not the number of lines actually rendered.
    pub fn num_lines(&self) -> usize {
        self.num_lines
    }

    /// Whether `line_index` is a document line configured as part of the header.
    /// Uses the configured extent, not the rendered one — see [`Self::num_lines`].
    pub fn contains(&self, line_index: usize) -> bool {
        line_index < self.num_lines
    }
}

fn build_rows(doc: &mut Document, size: &ViewportSize, num_lines: usize) -> Vec<Row> {
    // Reserve at least one non-header row so the header does not cover the entire viewport.
    let max_height = size.height().saturating_sub(1);
    rows::from_lines(doc, size.width(), 0..num_lines, max_height)
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

    #[test]
    fn no_header_when_num_lines_is_zero() {
        let mut doc = Document::from_string("a\nb\nc\n".into());
        let size = ViewportSize {
            width: 10,
            height: 5,
        };
        let h = Header::new(&mut doc, &size, 0);
        assert_eq!(h.height(), 0);
        assert!(h.rows().is_empty());
        assert!(!h.contains(0));
    }

    #[test]
    fn header_takes_first_lines() {
        let mut doc = Document::from_string("a\nb\nc\nd\n".into());
        let size = ViewportSize {
            width: 10,
            height: 5,
        };
        let h = Header::new(&mut doc, &size, 2);
        assert_eq!(pos(h.rows()), vec![(0, 0), (1, 0)]);
        assert_eq!(h.height(), 2);
    }

    #[test]
    fn contains_reflects_configured_range() {
        let mut doc = Document::from_string("a\nb\nc\nd\n".into());
        let size = ViewportSize {
            width: 10,
            height: 5,
        };
        let h = Header::new(&mut doc, &size, 2);
        assert!(h.contains(0));
        assert!(h.contains(1));
        assert!(!h.contains(2));
    }

    #[test]
    fn header_height_capped_to_leave_room_for_content() {
        // viewport height = 5 -> max header height is 4.
        let mut doc = Document::from_string("a\nb\nc\nd\ne\nf\n".into());
        let size = ViewportSize {
            width: 10,
            height: 5,
        };
        let h = Header::new(&mut doc, &size, 5);
        assert_eq!(h.height(), 4);
        // contains() reflects the configured num_lines, not the visible row count.
        assert!(h.contains(4));
    }

    #[test]
    fn header_includes_wrap_rows_of_long_lines() {
        // "abcd" at width 2 wraps to (0,0)=ab, (0,1)=cd.
        let mut doc = Document::from_string("abcd\ne\nf\ng\n".into());
        let size = ViewportSize {
            width: 2,
            height: 6,
        };
        let h = Header::new(&mut doc, &size, 1);
        assert_eq!(pos(h.rows()), vec![(0, 0), (0, 1)]);
    }

    #[test]
    fn resize_recomputes_rows_at_new_width() {
        let mut doc = Document::from_string("abcd\ne\n".into());
        let size = ViewportSize {
            width: 10,
            height: 5,
        };
        let mut h = Header::new(&mut doc, &size, 1);
        assert_eq!(pos(h.rows()), vec![(0, 0)]);

        let new_size = ViewportSize {
            width: 2,
            height: 5,
        };
        h.resize(&mut doc, &new_size);
        assert_eq!(pos(h.rows()), vec![(0, 0), (0, 1)]);
    }
}
