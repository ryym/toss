use std::cmp;

use ansi_control_codes::parser::{Token, TokenStream};
use unicode_width::UnicodeWidthChar;

/// Manage line wrappings. This class does not know about a terminal.
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
#[derive(Debug)]
pub(crate) struct LineWraps {
    rows: Vec<Row>,
    wraps: Vec<Wrap>,
}

impl LineWraps {
    pub(crate) fn new(lines: Vec<String>, n_cols: usize) -> Self {
        let mut rows = Vec::with_capacity(lines.len());
        let mut wraps = Vec::with_capacity(lines.len());

        for (i, line) in lines.into_iter().enumerate() {
            let mut line_slices = Vec::new();
            let mut n_cells = 0;
            let mut byte_idx = 0;
            let mut last_line_byte_idx = 0;

            let mut push_slice = |start_byte: usize, end_byte: usize| {
                rows.push(Row {
                    index: rows.len(),
                    wrap_index: i,
                    line_slice_index: line_slices.len(),
                });
                line_slices.push(LineSlice {
                    start_byte,
                    end_byte,
                });
            };

            for token in TokenStream::from(&line) {
                match token {
                    Token::ControlFunction(c) => {
                        // Allocate a string just to get the byte length of control codes.
                        byte_idx += String::from(c).len();
                    }
                    Token::String(s) => {
                        for c in s.chars() {
                            let cell_width = c.width().unwrap_or(0);
                            n_cells += cell_width;
                            if n_cells > n_cols {
                                push_slice(last_line_byte_idx, byte_idx);
                                n_cells = cell_width;
                                last_line_byte_idx = byte_idx;
                            }
                            byte_idx += c.len_utf8();
                        }
                    }
                }
            }

            if last_line_byte_idx < byte_idx || line.is_empty() {
                push_slice(last_line_byte_idx, byte_idx);
            }
            wraps.push(Wrap {
                original_line: line,
                slices: line_slices,
            });
        }

        Self { rows, wraps }
    }

    pub(crate) fn rows_len(&self) -> usize {
        self.rows.len()
    }

    pub(crate) fn original_lines_iter(
        &self,
        row_start: usize,
        row_end: usize,
    ) -> OriginalLineIter<'_> {
        let start = &self.rows[row_start];
        let end = &self.rows[row_end - 1];
        OriginalLineIter {
            wraps: &self.wraps[start.wrap_index..=end.wrap_index],
            start_line_slice_index: start.line_slice_index,
            end_line_slice_index: end.line_slice_index,
            index: 0,
        }
    }

    pub(crate) fn row_at(&self, index: usize) -> RowView<'_> {
        let row = &self.rows[index];
        let wrap = &self.wraps[row.wrap_index];
        let line_slice = &wrap.slices[row.line_slice_index];
        RowView {
            original_line: &wrap.original_line,
            line_slice,
            index: row.index,
            line_slice_index: row.line_slice_index,
            n_line_slices: wrap.slices.len(),
        }
    }

    pub(crate) fn slice_line(&self, row_start: usize, row_end: usize) -> &str {
        let start_row = &self.rows[row_start];
        let wrap = &self.wraps[start_row.wrap_index];

        let wrap_end_index = start_row.index + wrap.slices.len() - start_row.line_slice_index;
        let row_end_inclusive = cmp::min(row_end - 1, wrap_end_index);
        let end_line = &self.rows[row_end_inclusive];

        let slice_start = wrap.slices[start_row.line_slice_index].start_byte;
        let slice_end = wrap.slices[end_line.line_slice_index].end_byte;
        &wrap.original_line[slice_start..slice_end]
    }
}

#[derive(Debug)]
struct Row {
    index: usize,
    wrap_index: usize,
    line_slice_index: usize,
}

#[derive(Debug)]
struct Wrap {
    original_line: String,
    slices: Vec<LineSlice>,
}

#[derive(Debug)]
struct LineSlice {
    start_byte: usize,
    end_byte: usize,
}

#[derive(Debug)]
pub(crate) struct OriginalLineIter<'w> {
    wraps: &'w [Wrap],
    start_line_slice_index: usize,
    end_line_slice_index: usize, // inclusive
    index: usize,
}

#[derive(Debug)]
pub(crate) struct OriginalLineView<'w> {
    pub line: &'w str,
    pub n_rows: usize,
}

impl<'w> Iterator for OriginalLineIter<'w> {
    type Item = OriginalLineView<'w>;

    fn next(&mut self) -> Option<Self::Item> {
        let item = match self.wraps.get(self.index) {
            None => None,
            Some(wrap) => {
                let slice_start_idx = if self.index == 0 {
                    self.start_line_slice_index
                } else {
                    0
                };
                let slice_end_idx = if self.index == self.wraps.len() - 1 {
                    self.end_line_slice_index
                } else {
                    wrap.slices.len() - 1
                };
                let slice_start = &wrap.slices[slice_start_idx];
                let slice_end = &wrap.slices[slice_end_idx];
                let line = &wrap.original_line[slice_start.start_byte..slice_end.end_byte];
                Some(OriginalLineView {
                    line,
                    n_rows: slice_end_idx - slice_start_idx + 1,
                })
            }
        };
        self.index += 1;
        item
    }
}

pub(crate) struct RowView<'s> {
    original_line: &'s String,
    line_slice: &'s LineSlice,
    pub index: usize,
    pub line_slice_index: usize,
    pub n_line_slices: usize,
}

impl RowView<'_> {
    // Used in tests.
    #[allow(dead_code)]
    pub(crate) fn line(&self) -> &str {
        &self.original_line[self.line_slice.start_byte..self.line_slice.end_byte]
    }
}

#[cfg(test)]
mod tests_line_wrapping {
    use pretty_assertions::assert_eq;

    fn wrapped_lines(wraps: super::LineWraps) -> Vec<String> {
        let mut v = Vec::with_capacity(wraps.rows_len());
        for i in 0..wraps.rows_len() {
            let row = wraps.row_at(i);
            v.push(row.line().to_string());
        }
        v
    }

    #[test]
    fn dont_wrap_lines_if_width_is_enough() {
        let lines = vec![
            "abc".to_string(),
            "あいう".to_string(),
            "".to_string(),
            " ".to_string(),
        ];
        let wraps = super::LineWraps::new(lines, 10);
        assert_eq!(
            wrapped_lines(wraps),
            vec![
                "abc".to_string(),
                "あいう".to_string(),
                "".to_string(),
                " ".to_string(),
            ]
        );
    }

    #[test]
    fn wrap_overflowed_lines() {
        let lines = vec![
            "abc".to_string(),
            "abcd".to_string(),
            "abcdefghijk".to_string(),
        ];
        let wraps = super::LineWraps::new(lines, 3);
        assert_eq!(
            wrapped_lines(wraps),
            vec![
                // line 1
                "abc".to_string(),
                // line 2
                "abc".to_string(),
                "d".to_string(),
                // line 3
                "abc".to_string(),
                "def".to_string(),
                "ghi".to_string(),
                "jk".to_string(),
            ],
        );
    }

    #[test]
    fn wrap_overflowed_non_ascii_lines() {
        let lines = vec![
            "abcde".to_string(),
            "abcdef".to_string(),
            "あい".to_string(),
            "あいう".to_string(),
            "あいうえ".to_string(),
            "😀😇😎".to_string(),
        ];
        let wraps = super::LineWraps::new(lines, 5);
        assert_eq!(
            wrapped_lines(wraps),
            vec![
                "abcde".to_string(),
                "abcde".to_string(),
                "f".to_string(),
                "あい".to_string(),
                "あい".to_string(),
                "う".to_string(),
                "あい".to_string(),
                "うえ".to_string(),
                "😀😇".to_string(),
                "😎".to_string(),
            ],
        );
    }
}
