use std::ops::{Bound, RangeBounds};

use ansi_control_codes::parser::{Token, TokenStream};
use unicode_width::UnicodeWidthChar;

use crate::reader::LinePos;

/// A line in a page.
#[derive(Debug)]
pub(super) struct PageLine<Meta> {
    // XXX: reader への依存を作らないように generics にする案は悪くないと思うが、
    // 型引数が各所で必要になり、かつ型引数に指定する型は強制的にパブリックにせざるを得なくて、
    // メリットに合うかは微妙。テストではちょっと面倒だが、普通に LinePos もたせる方がまあいいか...
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

    // pub fn slice(&self, start_slice_idx: usize, end_slice_idx: usize) -> RowSpan<'_> {
    //     let slice_start = &self.wrap.slices[start_slice_idx];
    //     let slice_end = &self.wrap.slices[end_slice_idx];
    //     let s = &self.sentence.raw[slice_start.start_byte..slice_end.end_byte];
    //     let size = end_slice_idx - start_slice_idx + 1;
    //     RowSpan::new(s, size)
    // }

    pub fn slice(&self, range: impl RangeBounds<usize>) -> RowSpan<'_> {
        let start_slice_idx = match range.start_bound() {
            Bound::Unbounded => 0,
            Bound::Included(start) => *start,
            Bound::Excluded(_) => panic!("unexpected excluded bound for slice start"),
        };
        let end_slice_idx = match range.end_bound() {
            Bound::Unbounded => self.wrap.slices.len() - 1,
            Bound::Included(end) => *end,
            Bound::Excluded(end) => *end - 1,
        };
        let slice_start = &self.wrap.slices[start_slice_idx];
        let slice_end = &self.wrap.slices[end_slice_idx];
        let s = &self.sentence.raw[slice_start.start_byte..slice_end.end_byte];
        let size = end_slice_idx - start_slice_idx + 1;
        RowSpan::new(s, size)
    }

    pub fn wrap_in(&mut self, n_cols: usize) {
        self.wrap = Wrap::new(&self.sentence, n_cols);
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
        let mut i_raw_last_line_end = 0;
        for c in sentence.plain.chars() {
            let i_raw = sentence.plain_to_raw[i_plain_char];
            let cell_width = c.width().unwrap_or(0);
            n_cells += cell_width;
            if n_cells > n_cols {
                slices.push(LineSlice::new(i_raw_last_line_end, i_raw));
                n_cells = cell_width;
                i_raw_last_line_end = i_raw;
            }
            i_plain_char += c.len_utf8();
        }
        slices.push(LineSlice::new(i_raw_last_line_end, sentence.raw.len()));

        Self { slices }
    }
}

#[derive(Debug)]
struct LineSlice {
    /// A start position in the original raw line.
    pub start_byte: usize,
    /// An end position in the original raw line (exclusive).
    pub end_byte: usize,
}

impl LineSlice {
    fn new(start_byte: usize, end_byte: usize) -> Self {
        Self {
            start_byte,
            end_byte,
        }
    }
}

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
