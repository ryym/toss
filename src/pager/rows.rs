//! Reads runs of [`Row`]s out of a [`Document`] at a given width.
//!
//! A row that does not exist — past the end of the document, or not yet streamed in — is
//! simply absent from the result rather than an error, so every function here can return
//! fewer rows than asked for.

use std::ops::Range;

use crate::{document::Document, line::Row};

/// Build a list of [`Row`]s with `width` from lines in the given range.
/// It truncates rows by `max_rows`. In that case, a line might be cut off mid-way.
pub fn from_lines(
    doc: &mut Document,
    width: usize,
    line_index_range: Range<usize>,
    max_rows: usize,
) -> Vec<Row> {
    let mut rows = vec![];
    for i in line_index_range {
        if let Some(line) = doc.line(i) {
            rows.extend_from_slice(&line.wrap(width));
        }
        if rows.len() >= max_rows {
            break;
        }
    }
    rows.truncate(max_rows);
    rows
}

/// A row position in the document: `(line_index, wrap_index)`.
type RowPos = (usize, usize);

/// Build a list of at most `count` [`Row`]s with the given width, starting at `start` and
/// reading forward. Returns fewer rows when the document ends first.
pub fn list_forward(doc: &mut Document, width: usize, start: RowPos, count: usize) -> Vec<Row> {
    let mut rows = Vec::new();
    let (mut line_index, mut wrap_index) = start;
    while rows.len() < count {
        let line = match doc.line(line_index) {
            Some(l) => l,
            None => break,
        };
        let line_rows: Vec<Row> = line
            .wrap(width)
            .into_iter()
            .skip(wrap_index)
            .take(count - rows.len())
            .collect();
        rows.extend(line_rows);
        wrap_index = 0;
        line_index += 1;
    }
    rows
}

/// Where a backward read starts from.
pub enum DocPos<'row> {
    /// The last row of the document.
    End,
    /// The row just above the given one, which is itself excluded.
    Before(&'row Row),
}

/// Build a list of at most `count` [`Row`]s with the given width, reading backward from
/// the row `start` names. Returns fewer rows when the document begins first. The rows come
/// back in reading order, so the row `start` names is the last of them.
pub fn list_backward(doc: &mut Document, width: usize, start: DocPos, count: usize) -> Vec<Row> {
    let (mut line_index, mut from_wrap) = match start {
        DocPos::End => match doc.line_count().checked_sub(1) {
            Some(i) => (i, None),
            None => return vec![],
        },
        DocPos::Before(row) => {
            if row.wrap_index() == 0 {
                match row.line_index().checked_sub(1) {
                    Some(i) => (i, None),
                    None => return vec![],
                }
            } else {
                (row.line_index(), Some(row.wrap_index() - 1))
            }
        }
    };

    let mut rows = Vec::new();
    while rows.len() < count {
        let line = match doc.line(line_index) {
            Some(l) => l,
            None => break,
        };
        let line_rows = line.wrap(width);
        let wrap_index_rev = match from_wrap {
            None => 0,
            Some(w) => line_rows.len() - 1 - w,
        };
        let line_rows: Vec<Row> = line_rows
            .into_iter()
            .rev()
            .skip(wrap_index_rev)
            .take(count - rows.len())
            .collect();
        rows.extend(line_rows);
        from_wrap = None;
        match line_index.checked_sub(1) {
            Some(prev) => line_index = prev,
            None => break,
        }
    }
    rows.reverse();
    rows
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
    fn from_lines_collects_rows_in_range() {
        let mut doc = Document::from_string("a\nb\nc\nd\n".into());
        let rows = from_lines(&mut doc, 80, 0..4, 10);
        assert_eq!(pos(&rows), vec![(0, 0), (1, 0), (2, 0), (3, 0)]);
    }

    #[test]
    fn from_lines_truncates_at_max_rows() {
        let mut doc = Document::from_string("a\nb\nc\nd\n".into());
        let rows = from_lines(&mut doc, 80, 0..4, 2);
        assert_eq!(pos(&rows), vec![(0, 0), (1, 0)]);
    }

    #[test]
    fn from_lines_includes_wrap_rows() {
        // "abcde" wraps to (0,0)=ab, (0,1)=cd, (0,2)=e at width 2.
        let mut doc = Document::from_string("abcde\nf\n".into());
        let rows = from_lines(&mut doc, 2, 0..2, 10);
        assert_eq!(pos(&rows), vec![(0, 0), (0, 1), (0, 2), (1, 0)]);
    }

    #[test]
    fn from_lines_truncation_can_cut_mid_line() {
        let mut doc = Document::from_string("abcde\nf\n".into());
        // Only the first two wrap rows of line 0 fit.
        let rows = from_lines(&mut doc, 2, 0..2, 2);
        assert_eq!(pos(&rows), vec![(0, 0), (0, 1)]);
    }

    #[test]
    fn from_lines_returns_empty_for_out_of_range() {
        let mut doc = Document::from_string("a\nb\n".into());
        let rows = from_lines(&mut doc, 80, 5..10, 10);
        assert!(rows.is_empty());
    }

    #[test]
    fn list_forward_returns_rows_starting_at_position() {
        let mut doc = Document::from_string("a\nb\nc\nd\n".into());
        let rows = list_forward(&mut doc, 80, (1, 0), 2);
        assert_eq!(pos(&rows), vec![(1, 0), (2, 0)]);
    }

    #[test]
    fn list_forward_starts_from_mid_wrap() {
        // "abcde" wraps to (0,0), (0,1), (0,2) at width 2.
        let mut doc = Document::from_string("abcde\nf\n".into());
        let rows = list_forward(&mut doc, 2, (0, 1), 3);
        assert_eq!(pos(&rows), vec![(0, 1), (0, 2), (1, 0)]);
    }

    #[test]
    fn list_forward_stops_at_end_of_doc() {
        let mut doc = Document::from_string("a\nb\n".into());
        let rows = list_forward(&mut doc, 80, (0, 0), 10);
        assert_eq!(pos(&rows), vec![(0, 0), (1, 0)]);
    }

    #[test]
    fn list_backward_from_doc_end() {
        let mut doc = Document::from_string("a\nb\nc\n".into());
        let rows = list_backward(&mut doc, 80, DocPos::End, 2);
        assert_eq!(pos(&rows), vec![(1, 0), (2, 0)]);
    }

    #[test]
    fn list_backward_from_before_row() {
        let mut doc = Document::from_string("a\nb\nc\nd\n".into());
        let pivot = doc.line(2).unwrap().wrap(80)[0].clone();
        let rows = list_backward(&mut doc, 80, DocPos::Before(&pivot), 2);
        assert_eq!(pos(&rows), vec![(0, 0), (1, 0)]);
    }

    #[test]
    fn list_backward_from_before_wrapped_row() {
        // "abcde" wraps to (0,0), (0,1), (0,2) at width 2.
        let mut doc = Document::from_string("abcde\nf\n".into());
        let pivot = doc.line(0).unwrap().wrap(2)[2].clone();
        let rows = list_backward(&mut doc, 2, DocPos::Before(&pivot), 2);
        assert_eq!(pos(&rows), vec![(0, 0), (0, 1)]);
    }

    #[test]
    fn list_backward_returns_empty_at_doc_start() {
        let mut doc = Document::from_string("a\nb\n".into());
        let pivot = doc.line(0).unwrap().wrap(80)[0].clone();
        let rows = list_backward(&mut doc, 80, DocPos::Before(&pivot), 5);
        assert!(rows.is_empty());
    }

    #[test]
    fn list_backward_stops_at_doc_start_when_count_too_large() {
        let mut doc = Document::from_string("a\nb\nc\n".into());
        let rows = list_backward(&mut doc, 80, DocPos::End, 10);
        assert_eq!(pos(&rows), vec![(0, 0), (1, 0), (2, 0)]);
    }
}
