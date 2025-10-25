use unicode_width::UnicodeWidthChar;

use crate::lines::Line;

pub(super) fn wrap_lines(lines: Vec<Line>, n_cols: usize) -> (Vec<Row>, Vec<Wrap>) {
    let mut rows = Vec::with_capacity(lines.len());
    let mut wraps = Vec::with_capacity(lines.len());

    for (i, line) in lines.into_iter().enumerate() {
        let mut line_slices = Vec::new();
        let mut push_slice = |start_byte: usize, end_byte: usize| {
            rows.push(Row {
                wrap_index: i,
                line_slice_index: line_slices.len(),
            });
            line_slices.push(LineSlice {
                start_byte,
                end_byte,
            });
        };

        let mut n_cells = 0;
        let mut i_plain_char = 0;
        let mut i_raw_last_line_end = 0;
        for c in line.plain.chars() {
            let i_raw = line.plain_to_raw[i_plain_char];
            let cell_width = c.width().unwrap_or(0);
            n_cells += cell_width;
            if n_cells > n_cols {
                push_slice(i_raw_last_line_end, i_raw);
                n_cells = cell_width;
                i_raw_last_line_end = i_raw;
            }
            i_plain_char += c.len_utf8();
        }
        push_slice(i_raw_last_line_end, line.raw.len());

        wraps.push(Wrap { line, line_slices });
    }

    (rows, wraps)
}

#[derive(Debug)]
pub(super) struct Row {
    pub wrap_index: usize,
    pub line_slice_index: usize,
}

#[derive(Debug)]
pub(super) struct Wrap {
    pub line: Line,
    pub line_slices: Vec<LineSlice>,
}

impl Wrap {
    pub(super) fn slice_line(&self, start_slice_idx: usize, end_slice_idx: usize) -> RowSpan<'_> {
        let slice_start = &self.line_slices[start_slice_idx];
        let slice_end = &self.line_slices[end_slice_idx];
        let line = &self.line.raw[slice_start.start_byte..slice_end.end_byte];
        let size = end_slice_idx - start_slice_idx + 1;
        RowSpan::new(line, size)
    }
}

#[derive(Debug)]
pub(super) struct LineSlice {
    pub start_byte: usize,
    pub end_byte: usize,
}

#[derive(Debug, PartialEq)]
pub(crate) struct RowSpan<'w> {
    line: &'w str,
    size: usize,
}

impl<'w> RowSpan<'w> {
    pub(super) fn new(line: &'w str, size: usize) -> Self {
        Self { line, size }
    }

    #[inline]
    pub(crate) fn line(&self) -> &str {
        self.line
    }

    #[inline]
    pub(crate) fn size(&self) -> usize {
        self.size
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::lines::Line;

    fn line_rows(n_cols: usize, lines: Vec<String>) -> Vec<String> {
        let lines = lines.into_iter().map(Line::new).collect();
        let (rows, wraps) = wrap_lines(lines, n_cols);
        rows.iter()
            .map(|row| {
                let wrap = &wraps[row.wrap_index];
                let line_slice = &wrap.line_slices[row.line_slice_index];
                wrap.line.raw[line_slice.start_byte..line_slice.end_byte].to_string()
            })
            .collect()
    }

    #[test]
    fn dont_wrap_lines_if_width_is_enough() {
        let lines = vec![
            "abc".to_string(),
            "あいう".to_string(),
            "".to_string(),
            " ".to_string(),
        ];
        assert_eq!(
            line_rows(10, lines),
            vec![
                "abc".to_string(),
                "あいう".to_string(),
                "".to_string(),
                " ".to_string(),
            ]
        );
    }

    #[test]
    fn wrap_lines_without_caring_escape_sequences() {
        let lines = vec![
            "ab\u{1b}[31mc\u{1b}[0md".to_string(),
            "\u{1b}[31mabc\u{1b}[0md".to_string(),
            "\u{1b}[31mabcd\u{1b}[0m".to_string(),
        ];
        assert_eq!(line_rows(4, lines.clone()), lines);
    }

    #[test]
    fn wrap_overflowed_lines() {
        let lines = vec![
            "abc".to_string(),
            "abcd".to_string(),
            "abcdefghijk".to_string(),
        ];
        assert_eq!(
            line_rows(3, lines),
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
        assert_eq!(
            line_rows(5, lines),
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
