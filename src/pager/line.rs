use std::ops::{Bound, RangeBounds};

use unicode_width::UnicodeWidthChar;

use crate::line::Line;

/// A line in a page.
///
/// ## About line wrapping on terminal
/// Modern terminal emulators can wrap a long line automatically if it overflows the width.
/// But it requires extra consideration when we communicate with a terminal in the raw mode.
/// For example, if you write a long line at row 1 it continues to row 2 or more until it ends.
/// In this case, the next line needs to be written after the last row fo the first line,
/// not simply at row 2.
///
/// Alternatively, it is also possible to wrap lines by ourselves and keep all lines
/// always fit in the terminal width so that a terminal doesn't automatically wrap lines.
/// This is simpler than the first approach. But in this approach, a terminal cannot know
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
///           Lorem ipsum dolor sit amet, consectetur elit.  ───┬─ line
///           A finibus massa ultricies nec.                 ───┘
///
///           ┌ wrap
///           ├ - - - - - - - - - - - ┐
///  row ─┬── ❘ Lorem ipsum dolor si  ❘ ──┬─ wrap row
///       ├── ❘ t amet, consectetur   ❘ ──┤
///       ├── ❘ elit.                 ❘ ──┘
///       │   └ - - - - - - - - - - - ┘
///       │   ┌ - - - - - - - - - - - ┐
///       ├── ❘ A finibus massa ultr  ❘ ──┬─ wrap row
///       └── ❘ icies nec.            ❘ ──┘
///           └ - - - - - - - - - - - ┘
/// ```
#[derive(Debug)]
pub(super) struct PageLine<Meta> {
    /// Any metadata related to this line.
    meta: Meta,

    /// An original line.
    line: Line,

    /// Applied wrapping based on the page's column size.
    wrap: Wrap,
}

impl<Meta> PageLine<Meta> {
    pub fn new(meta: Meta, text: String, n_cols: usize) -> Self {
        let line = Line::new(text);
        let wrap = Wrap::new(&line, n_cols);
        Self { meta, line, wrap }
    }

    #[inline]
    pub fn meta(&self) -> &Meta {
        &self.meta
    }

    #[inline]
    pub fn row_len(&self) -> usize {
        self.wrap.rows.len()
    }

    #[inline]
    pub fn plain(&self) -> &str {
        self.line.plain()
    }

    /// Return a row span which spans wrap rows of the given indice.
    pub fn slice(&self, wrap_row_range: impl RangeBounds<usize>) -> RowSpan<'_> {
        let (start_byte, end_byte, row_len) = self.wrap.row_span_ends(wrap_row_range);
        let s = &self.line.raw()[start_byte..end_byte];
        RowSpan::new(s, row_len)
    }
}

#[derive(Debug, Default)]
struct Wrap {
    /// Rows are a list of cut-off text of the original line to fit in the page width.
    rows: Vec<WrapRow>,
}

impl Wrap {
    fn new(line: &Line, n_cols: usize) -> Self {
        let mut rows = Vec::new();

        let mut n_cells = 0;
        let mut i_plain_char = 0;
        for c in line.plain().chars() {
            let i_raw = line.plain_to_raw()[i_plain_char];
            let cell_width = c.width().unwrap_or(0);
            n_cells += cell_width;
            if n_cells > n_cols {
                rows.push(WrapRow::new(i_raw));
                n_cells = cell_width;
            }
            i_plain_char += c.len_utf8();
        }
        rows.push(WrapRow::new(line.raw().len()));

        Self { rows }
    }

    fn row_span_ends(&self, wrap_row_range: impl RangeBounds<usize>) -> (usize, usize, usize) {
        let start_row_idx = match wrap_row_range.start_bound() {
            Bound::Unbounded => 0,
            Bound::Included(start) => *start,
            Bound::Excluded(_) => panic!("unexpected excluded bound for wrap row start"),
        };
        let end_row_idx = match wrap_row_range.end_bound() {
            Bound::Unbounded => self.rows.len() - 1,
            Bound::Included(end) => *end,
            Bound::Excluded(end) => *end - 1,
        };
        let start_byte = if start_row_idx == 0 {
            0
        } else {
            self.rows[start_row_idx - 1].end_byte
        };
        let end_row = &self.rows[end_row_idx];
        let row_len = end_row_idx - start_row_idx + 1;
        (start_byte, end_row.end_byte, row_len)
    }
}

#[derive(Debug)]
struct WrapRow {
    /// An end position in the original raw line (exclusive).
    end_byte: usize,
}

impl WrapRow {
    fn new(end_byte: usize) -> Self {
        Self { end_byte }
    }
}

/// Iterator of row spans of the current page.
/// A row span is neither a row nor a line. It is a wrapped line possibly cut off in the middle
/// to fit in the page. For example, if the first line of the source text is "aabbcc" and the
/// page column size is 2, the line will be broken into 3 rows: "aa", "bb", "cc". If the page
/// is scrolled down one row, the first two rows will be "bb" and "cc". This is a row span.
#[derive(Debug, PartialEq)]
pub(crate) struct RowSpan<'l> {
    line: &'l str,
    size: usize,
}

impl<'line> RowSpan<'line> {
    pub(super) fn new(line: &'line str, size: usize) -> Self {
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

#[cfg(all(feature = "bench", test))]
mod bench {
    extern crate test;

    use test::Bencher;

    use crate::pager::PageLine;

    // A string with ASCI escape sequences. The plain content is below:
    // 77   │                 let start_byte = cmp::max(block.start_byte(), self.content_start_byte.unwrap_or(0));
    const S: &str = "[38;2;116;115;105m  77[0m   [38;2;116;115;105m│[0m [38;2;171;178;191m                [0m[38;2;198;120;221mlet[0m[38;2;171;178;191m start_byte [0m[38;2;171;178;191m=[0m[38;2;171;178;191m [0m[38;2;171;178;191mcmp[0m[38;2;171;178;191m::[0m[38;2;171;178;191mmax[0m[38;2;171;178;191m([0m[38;2;171;178;191mblock[0m[38;2;171;178;191m.[0m[38;2;86;182;194mstart_byte[0m[38;2;171;178;191m([0m[38;2;171;178;191m)[0m[38;2;171;178;191m,[0m[38;2;171;178;191m [0m[38;2;224;108;117mself[0m[38;2;171;178;191m.[0m[38;2;171;178;191mcontent_start_byte[0m[38;2;171;178;191m.[0m[38;2;86;182;194munwrap_or[0m[38;2;171;178;191m([0m[38;2;209;154;102m0[0m[38;2;171;178;191m)[0m[38;2;171;178;191m)[0m[38;2;171;178;191m;[0m";

    #[bench]
    fn make_line(b: &mut Bencher) {
        b.iter(|| PageLine::new((), S.to_string(), 17))
    }
}
