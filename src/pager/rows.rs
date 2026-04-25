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

/// (line_index, wrap_index)
type RowPos = (usize, usize);

/// Build a list of [`Row`]s with the given width from `start` up to `count`.
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

pub enum DocPos<'row> {
    End,
    Before(&'row Row),
}

/// Build a list of [`Row`]s with the given width from `start` up to `count` in the reversed order.
pub fn list_backward(doc: &mut Document, width: usize, start: DocPos, count: usize) -> Vec<Row> {
    let (mut line_index, mut from_wrap) = match start {
        DocPos::End => ((doc.line_count() as isize) - 1, None),
        DocPos::Before(row) => {
            if row.wrap_index() == 0 {
                if row.line_index() == 0 {
                    return vec![];
                }
                ((row.line_index() as isize) - 1, None)
            } else {
                (row.line_index() as isize, Some(row.wrap_index() - 1))
            }
        }
    };

    let mut rows = Vec::new();
    while rows.len() < count && line_index >= 0 {
        let line = match doc.line(line_index as usize) {
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
        line_index -= 1;
    }
    rows.reverse();
    rows
}
