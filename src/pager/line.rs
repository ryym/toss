use std::ops::{Bound, RangeBounds};

use ansi_control_codes::parser::{Token, TokenStream};
use unicode_width::UnicodeWidthChar;

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
///           Lorem ipsum dolor sit amet, consectetur elit.  ───┬─ sentence
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
pub(super) struct PageLine<Meta> {
    /// Any metadata related to this line.
    meta: Meta,

    /// A original sentence this struct refers to.
    sentence: Sentence,

    /// Wrapping information based on the window width.
    wrap: Wrap,
}

impl<Meta> PageLine<Meta> {
    pub fn new(meta: Meta, text: String, n_cols: usize) -> Self {
        let sentence = Sentence::new(text);
        let wrap = Wrap::new(&sentence, n_cols);
        Self {
            meta,
            sentence,
            wrap,
        }
    }

    #[inline]
    pub fn meta(&self) -> &Meta {
        &self.meta
    }

    #[inline]
    pub fn row_len(&self) -> usize {
        self.wrap.slices.len()
    }

    pub fn slice(&self, slice_range: impl RangeBounds<usize>) -> RowSpan<'_> {
        let (start_byte, end_byte, row_len) = self.wrap.row_span_ends(slice_range);
        let s = &self.sentence.raw[start_byte..end_byte];
        RowSpan::new(s, row_len)
    }
}

#[derive(Debug, Default)]
struct Sentence {
    /// An original line including ANSI escape sequences.
    pub raw: String,

    /// A plain text line without escape sequences.
    pub plain: String,

    /// A byte index mapping from each character in `plain` to `raw`. Example:
    /// - raw: `\xb[1mHi\xb[0m, 😀` ("Hi" is bold)
    /// - plain: `Hi, 😀`
    /// - plain_to_raw:
    ///   ```text
    ///   (escape sequences: bold start)
    ///   0:  4 ──── H
    ///   1:  5 ──── i
    ///   (escape sequences: reset)
    ///   2: 10 ──── ,
    ///   3: 11 ──── (space)
    ///   4: 12 ──┬─ 😀
    ///   5: 13 ──┤
    ///   6: 14 ──┤
    ///   7: 15 ──┘
    ///   ```
    pub plain_to_raw: Vec<usize>,
}

impl Sentence {
    fn new(raw_line: String) -> Self {
        let mut plain = String::with_capacity(raw_line.len());
        let mut plain_to_raw = Vec::new();
        let mut i_raw = 0;
        for token in TokenStream::from(&raw_line) {
            match token {
                Token::ControlFunction(c) => {
                    // Allocate a string just to get the byte length of escape sequences :(
                    i_raw += String::from(c).len();
                }
                Token::String(s) => {
                    plain.push_str(s);
                    for _ in s.as_bytes() {
                        plain_to_raw.push(i_raw);
                        i_raw += 1;
                    }
                }
            }
        }
        Self {
            raw: raw_line,
            plain,
            plain_to_raw,
        }
    }
}

#[derive(Debug, Default)]
struct Wrap {
    /// Slices to fit the original line in the window width.
    pub slices: Vec<LineSlice>,
}

impl Wrap {
    fn new(sentence: &Sentence, n_cols: usize) -> Self {
        let mut slices = Vec::new();

        let mut n_cells = 0;
        let mut i_plain_char = 0;
        for c in sentence.plain.chars() {
            let i_raw = sentence.plain_to_raw[i_plain_char];
            let cell_width = c.width().unwrap_or(0);
            n_cells += cell_width;
            if n_cells > n_cols {
                slices.push(LineSlice::new(i_raw));
                n_cells = cell_width;
            }
            i_plain_char += c.len_utf8();
        }
        slices.push(LineSlice::new(sentence.raw.len()));

        Self { slices }
    }

    fn row_span_ends(&self, slice_range: impl RangeBounds<usize>) -> (usize, usize, usize) {
        let start_slice_idx = match slice_range.start_bound() {
            Bound::Unbounded => 0,
            Bound::Included(start) => *start,
            Bound::Excluded(_) => panic!("unexpected excluded bound for slice start"),
        };
        let end_slice_idx = match slice_range.end_bound() {
            Bound::Unbounded => self.slices.len() - 1,
            Bound::Included(end) => *end,
            Bound::Excluded(end) => *end - 1,
        };
        let start_byte = if start_slice_idx == 0 {
            0
        } else {
            self.slices[start_slice_idx - 1].end_byte
        };
        let slice_end = &self.slices[end_slice_idx];
        let row_len = end_slice_idx - start_slice_idx + 1;
        (start_byte, slice_end.end_byte, row_len)
    }
}

#[derive(Debug)]
struct LineSlice {
    /// An end position in the original raw line (exclusive).
    end_byte: usize,
}

impl LineSlice {
    fn new(end_byte: usize) -> Self {
        Self { end_byte }
    }
}

/// Iterator of row spans of the current page.
/// A row span is neither a row nor a line. It is a wrapped line possibly cut off in the middle
/// to fit in the page. For example, if the first sentence of the source text is "aabbcc" and the
/// page column size is 2, the sentence will be broken into 3 rows: "aa", "bb", "cc". If the page
/// is scrolled down one row, the page will start with "bb" and "cc". This "bbcc" is a row span.
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
