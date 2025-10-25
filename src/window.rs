mod wraps;

use std::cmp;

use crate::{
    lines::Line,
    window::wraps::{wrap_lines, Row, Wrap},
};

pub(crate) use wraps::RowSpan;

/// The `Window` struct holds text to display and manages two things:
///
/// 1. Determine lines that are currently "visible" in the specified size of rows.
/// 1. Wrap lines based on the specified size of columns.
///
/// But this struct itself does not communicate with any IO.
///
/// ## About line wrapping on terminal
/// Modern terminal emulators can wrap a long line automatically if it overflows the width.
/// But it requires extra consideration when we communicate with a terminal in the raw mode.
/// For example, if you write a long line at row 1 (`termion::cursor::Goto(1, 1)`),
/// the line continues to row 2 or more until it ends. In this case, we need to
/// write the next line after the last row of the first line, not simply `Goto(1, 2)`.
///
/// Alternatively, it is also possible to wrap lines by ourselves and keep all lines
/// always fit in the terminal width so that a terminal doesn't automatically wrap lines.
/// This is simpler than the first approach which lets a terminal wrap lines while
/// tracking how lines are wrapped. But in the second approach, a terminal cannot know
/// whether each line actually continues. This affects a behavior of copying.
///
/// If a line is automatically wrapped by a terminal, you can copy it as if it is not wrapped:
/// ```text
///  │Let's say this line is au│
///  │tomatically wrapped.     │
/// ```
/// Copied text:
/// ```text
///  Let's say this line is automatically wrapped.
/// ```
/// But if it is wrapped by ourselves, it is copied with a line break:
/// ```text
///  │If this line is wrapped manually by th│
///  │e program, it is copied as two lines. │
/// ```
///
/// Toss implements the former behavior.
/// It lets a terminal wrap lines while keeping which point a line is wrapped at.
///
/// ## Terminology
/// ```text
///           Lorem ipsum dolor sit amet, consectetur elit.  ───┬─ (original) line
///           A finibus massa ultricies nec.                 ───┘
///
///           ┌ wrap
///           ├ - - - - - - - - - - - ┐
///  row ─┬── ❘ Lorem ipsum dolor si  ❘ ──┬─ line slice (per wrap)
///       ├── ❘ t amet, consectetur   ❘ ──┤
///       ├── ❘ elit.                 ❘ ──┘
///       │   └ - - - - - - - - - - - ┘
///       │   ┌ - - - - - - - - - - - ┐
///       ├── ❘ A finibus massa ultr  ❘ ──┬─ line slice (per wrap)
///       └── ❘ icies nec.            ❘ ──┘
///           └ - - - - - - - - - - - ┘
/// ```
#[derive(Debug, Default)]
pub(crate) struct Window {
    n_rows: usize,
    start_row_index: usize,
    rows: Vec<Row>,
    wraps: Vec<Wrap>,
}

impl Window {
    pub(crate) fn new(n_cols: usize, n_rows: usize, lines: Vec<Line>) -> Self {
        debug_assert!(!lines.is_empty());
        let (rows, wraps) = wrap_lines(lines, n_cols);
        Self {
            n_rows,
            start_row_index: 0,
            rows,
            wraps,
        }
    }

    #[inline]
    pub(crate) fn n_rows(&self) -> usize {
        self.n_rows
    }

    fn end_row_index(&self) -> usize {
        let n_rows = cmp::min(self.n_rows, self.rows.len());
        self.start_row_index + n_rows - 1 // inclusive
    }

    pub(crate) fn row_spans(&self) -> RowSpanIter<'_> {
        let start_row = &self.rows[self.start_row_index];
        let end_row = &self.rows[self.end_row_index()];
        RowSpanIter {
            wraps: &self.wraps[start_row.wrap_index..=end_row.wrap_index],
            start_line_slice_index: start_row.line_slice_index,
            end_line_slice_index: end_row.line_slice_index,
            cursor: 0,
        }
    }

    pub(crate) fn scroll_up_one_row(&mut self) -> bool {
        if self.start_row_index == 0 {
            return false;
        }
        self.start_row_index -= 1;
        true
    }

    pub(crate) fn scroll_down_one_row(&mut self) -> bool {
        if self.end_row_index() >= self.rows.len() - 1 {
            return false;
        }
        self.start_row_index += 1;
        true
    }

    pub(crate) fn start_row_span(&self) -> RowSpan<'_> {
        let new_row = &self.rows[self.start_row_index];
        let wrap = &self.wraps[new_row.wrap_index];
        wrap.slice_line(new_row.line_slice_index, wrap.line_slices.len() - 1)
    }

    pub(crate) fn end_row_span(&self) -> RowSpan<'_> {
        let new_row = &self.rows[self.end_row_index()];
        let wrap = &self.wraps[new_row.wrap_index];
        wrap.slice_line(0, new_row.line_slice_index)
    }

    pub(crate) fn seek_to_start(&mut self) {
        self.start_row_index = 0;
    }

    pub(crate) fn seek_to_end(&mut self) {
        self.start_row_index = self.rows.len() - self.n_rows;
    }
}

#[derive(Debug)]
pub(crate) struct RowSpanIter<'w> {
    wraps: &'w [Wrap],
    start_line_slice_index: usize,
    end_line_slice_index: usize, // inclusive
    cursor: usize,
}

impl<'w> Iterator for RowSpanIter<'w> {
    type Item = RowSpan<'w>;

    fn next(&mut self) -> Option<Self::Item> {
        let item = match self.wraps.get(self.cursor) {
            None => None,
            Some(wrap) => {
                let start_slice_idx = if self.cursor == 0 {
                    self.start_line_slice_index
                } else {
                    0
                };
                let end_slice_idx = if self.cursor == self.wraps.len() - 1 {
                    self.end_line_slice_index
                } else {
                    wrap.line_slices.len() - 1
                };
                Some(wrap.slice_line(start_slice_idx, end_slice_idx))
            }
        };
        self.cursor += 1;
        item
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn row_spans_many_lines() {
        let lines = vec!["abc", "def", "ghi", "jkl", "mno"];
        let lines = lines
            .into_iter()
            .map(|l| Line::new(l.to_string()))
            .collect();
        let mut win = Window::new(3, 3, lines);
        assert_eq!(
            win.row_spans().collect::<Vec<_>>(),
            vec![
                RowSpan::new("abc", 1),
                RowSpan::new("def", 1),
                RowSpan::new("ghi", 1),
            ]
        );
        win.scroll_down_one_row();
        assert_eq!(
            win.row_spans().collect::<Vec<_>>(),
            vec![
                RowSpan::new("def", 1),
                RowSpan::new("ghi", 1),
                RowSpan::new("jkl", 1),
            ]
        );
    }
}
