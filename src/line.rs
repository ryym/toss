//! Line representation with ANSI-aware plain/raw duality.
//!
//! Source lines may contain ANSI escape sequences (colors, bold, etc.). These must
//! be preserved in output but ignored for width calculation and search matching.
//! Each `Line` therefore maintains two views:
//!
//! - **Raw text**: the original bytes including escapes, used for rendering.
//! - **Plain text**: escape-stripped text, used for width calculation, wrapping, and search.
//!
//! A **plain_to_raw mapping** (byte-level) connects the two: given a byte position in
//! plain text, you can look up the corresponding position in raw text. This mapping is
//! how wrapping and search highlighting work correctly in the presence of escape sequences.
//! See the `plain_to_raw` field documentation for a concrete example.

use std::{cmp::Ordering, ops::Range};

use regex::Regex;
use unicode_width::UnicodeWidthChar;

use crate::ansi;

/// A single row produced by wrapping a [`Line`] at a given width.
///
/// When a line is too wide for the given width, it is split into multiple rows.
/// Each `Row` represents one of those wrap segments, carrying enough information
/// to locate the corresponding text in the original line.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Row {
    line_index: usize,
    wrap_index: usize,
    raw_range: Range<usize>,
}

impl Ord for Row {
    fn cmp(&self, other: &Self) -> Ordering {
        let ordering = self.line_index.cmp(&other.line_index);
        match ordering {
            Ordering::Equal => self.wrap_index.cmp(&other.wrap_index),
            _ => ordering,
        }
    }
}

impl PartialOrd for Row {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Row {
    /// Index of the source line in the document.
    #[inline]
    pub fn line_index(&self) -> usize {
        self.line_index
    }

    /// Zero-based position of this row among the wrap rows of its line.
    #[inline]
    pub fn wrap_index(&self) -> usize {
        self.wrap_index
    }

    /// Byte range in the line's raw text covered by this row.
    #[inline]
    pub fn raw_range(&self) -> &Range<usize> {
        &self.raw_range
    }
}

impl Row {
    fn new(line_index: usize, wrap_index: usize, raw_range: Range<usize>) -> Self {
        Self {
            line_index,
            wrap_index,
            raw_range,
        }
    }
}

/// Position of a specific search match: line index and a text range in the line.
#[derive(Debug, Clone, PartialEq)]
pub struct MatchPosition {
    pub line_index: usize,
    /// A match range in the line's plain text.
    pub plain_range: Range<usize>,
}

/// Specifies the starting position when searching within a [`Line`].
/// Behavior is undefined if a [`Row`] or [`MatchPosition`] for a different
/// line index is passed.
#[derive(Debug)]
pub enum SearchLineFrom<'a> {
    Start,
    /// Search from the given [`Row`] onward.
    Row(&'a Row),
    /// Search after the given match. The match itself is excluded.
    NextOf(&'a MatchPosition),
    /// Search backward before the given match. The match itself is excluded.
    PrevOf(&'a MatchPosition),
}

/// A single line of text from the document.
#[derive(Debug, Clone)]
pub struct Line {
    /// Zero-based position of this line in the document (i.e., the line number).
    /// This is passed through to [`Row::line_index`] when wrapping.
    index: usize,

    /// Original text including ANSI escape sequences.
    raw: String,

    /// Plain text without escape sequences.
    plain: String,

    /// Byte index mapping from each byte in `plain` to the corresponding byte
    /// position in `raw`. For multi-byte characters every byte of the character
    /// is mapped individually.
    ///
    /// Example:
    /// - raw:   `\x1b[1mHi\x1b[0m, 😀` ("Hi" is bold)
    /// - plain: `Hi, 😀`
    /// - plain_to_raw: [4, 5, 10, 11, 12, 13, 14, 15]
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
    plain_to_raw: Vec<usize>,
}

impl Line {
    /// Create a line from raw text, parsing out ANSI escape sequences.
    /// `index` is the position of this line in the document.
    pub fn new(index: usize, raw_line: String) -> Self {
        let mut plain = String::with_capacity(raw_line.len());
        let mut plain_to_raw = Vec::new();
        let mut i_raw = 0;
        for part in ansi::parse_text(&raw_line) {
            match part {
                ansi::Text::Control(s) => {
                    i_raw += s.len();
                }
                ansi::Text::Plain(s) => {
                    plain.push_str(s);
                    for _ in s.as_bytes() {
                        plain_to_raw.push(i_raw);
                        i_raw += 1;
                    }
                }
            }
        }
        Self {
            index,
            raw: raw_line,
            plain,
            plain_to_raw,
        }
    }

    /// Returns the position of this line in the document.
    #[cfg(test)]
    #[inline]
    pub fn index(&self) -> usize {
        self.index
    }

    /// Returns the original raw text including ANSI escape sequences.
    #[inline]
    pub fn raw(&self) -> &str {
        &self.raw
    }

    /// Compute the wrapped rows at the given width.
    ///
    /// Wrapping is computed on the plain text (visible characters only) but
    /// raw_range in each Row refers to the raw text so that slicing produces
    /// correct output including escape sequences.
    pub fn wrap(&self, width: usize) -> Vec<Row> {
        if width == 0 {
            return vec![Row {
                line_index: self.index,
                wrap_index: 0,
                raw_range: 0..self.raw.len(),
            }];
        }

        let mut rows = Vec::new();
        let mut row_start = 0;
        let mut col = 0;
        let mut i_plain_byte = 0;

        for ch in self.plain.chars() {
            let i_raw = self.plain_to_raw[i_plain_byte];
            let ch_width = ch.width().unwrap_or(0);
            if col + ch_width > width && col > 0 {
                rows.push(Row::new(self.index, rows.len(), row_start..i_raw));
                row_start = i_raw;
                col = ch_width;
            } else {
                col += ch_width;
            }
            i_plain_byte += ch.len_utf8();
        }
        rows.push(Row::new(self.index, rows.len(), row_start..self.raw.len()));

        rows
    }

    /// Number of screen rows this line occupies at the given width.
    pub fn row_count(&self, width: usize) -> usize {
        self.wrap(width).len()
    }

    /// Check if the plain text contains any match for the given regex.
    pub fn has_match(&self, query: &Regex) -> bool {
        query.is_match(&self.plain)
    }

    /// Find all matches of a regex in the plain text.
    pub fn find_matches(&self, query: &Regex) -> Vec<MatchPosition> {
        self.matches_iter(query, SearchLineFrom::Start).collect()
    }

    /// Find the first match in the plain text based on [`SearchLineFrom`].
    pub fn find_first_match_from(
        &self,
        query: &Regex,
        from: SearchLineFrom,
    ) -> Option<MatchPosition> {
        self.matches_iter(query, from).next()
    }

    /// Find the last match in the plain text based on [`SearchLineFrom`].
    pub fn find_last_match_from(
        &self,
        query: &Regex,
        from: SearchLineFrom,
    ) -> Option<MatchPosition> {
        self.matches_iter(query, from).last()
    }

    fn matches_iter<'a>(
        &'a self,
        query: &'a Regex,
        from: SearchLineFrom<'a>,
    ) -> Box<dyn Iterator<Item = MatchPosition> + 'a> {
        let matches = query.find_iter(&self.plain).map(|m| MatchPosition {
            line_index: self.index,
            plain_range: m.start()..m.end(),
        });
        match from {
            SearchLineFrom::Start => Box::new(matches),
            SearchLineFrom::NextOf(border) => {
                Box::new(matches.skip_while(|m| m.plain_range.start < border.plain_range.end))
            }
            SearchLineFrom::PrevOf(border) => {
                Box::new(matches.take_while(|m| m.plain_range.end <= border.plain_range.start))
            }
            SearchLineFrom::Row(row) => {
                Box::new(matches.skip_while(|m| {
                    self.plain_to_raw_safe(m.plain_range.start) < row.raw_range.start
                }))
            }
        }
    }

    /// Raw-text byte range corresponding to the match.
    /// The end falls back to `raw.len()` when `plain_range.end == plain.len()`.
    ///
    /// Behavior is undefined if `m` was not produced from this line.
    pub fn match_raw_range(&self, m: &MatchPosition) -> Range<usize> {
        self.plain_to_raw_safe(m.plain_range.start)..self.plain_to_raw_safe(m.plain_range.end)
    }

    /// Iterate raw byte positions where ANSI escape sequences are embedded
    /// inside the match. Each yielded index points to the raw byte where plain
    /// text resumes immediately after such an embedded escape sequence.
    ///
    /// Behavior is undefined if `m` was not produced from this line.
    pub fn match_inner_escape_boundaries<'a>(
        &'a self,
        m: &'a MatchPosition,
    ) -> impl Iterator<Item = usize> + 'a {
        self.plain_to_raw[m.plain_range.start..m.plain_range.end]
            .windows(2)
            .filter_map(|w| (w[1] - w[0] > 1).then_some(w[1]))
    }

    /// Zero-width regex match at the end of plain text like `$` yields
    /// a plain text index of `plain.len()`, which is out of bounds for `plain_to_raw`.
    /// This method treats it as the end of raw text.
    fn plain_to_raw_safe(&self, plain_index: usize) -> usize {
        self.plain_to_raw
            .get(plain_index)
            .copied()
            .unwrap_or(self.raw.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wrap_row_text(line: &Line, width: usize, wrap_index: usize) -> &str {
        let rows = line.wrap(width);
        &line.raw[rows[wrap_index].raw_range.clone()]
    }

    #[test]
    fn wrap_short_line() {
        let line = Line::new(0, "hello".into());
        assert_eq!(line.row_count(80), 1);
        assert_eq!(wrap_row_text(&line, 80, 0), "hello");
    }

    #[test]
    fn wrap_exact_width() {
        let line = Line::new(0, "abcde".into());
        assert_eq!(line.row_count(5), 1);
        assert_eq!(wrap_row_text(&line, 5, 0), "abcde");
    }

    #[test]
    fn wrap_overflow() {
        let line = Line::new(0, "abcdefgh".into());
        assert_eq!(line.row_count(5), 2);
        assert_eq!(wrap_row_text(&line, 5, 0), "abcde");
        assert_eq!(wrap_row_text(&line, 5, 1), "fgh");
    }

    #[test]
    fn wrap_multiple_rows() {
        let line = Line::new(0, "abcdefghijklm".into());
        assert_eq!(line.row_count(5), 3);
        assert_eq!(wrap_row_text(&line, 5, 0), "abcde");
        assert_eq!(wrap_row_text(&line, 5, 1), "fghij");
        assert_eq!(wrap_row_text(&line, 5, 2), "klm");
        // Multi-row range covers consecutive rows
        let rows = line.wrap(5);
        assert_eq!(
            &line.raw[rows[0].raw_range.start..rows[1].raw_range.end],
            "abcdefghij"
        );
        assert_eq!(
            &line.raw[rows[1].raw_range.start..rows[2].raw_range.end],
            "fghijklm"
        );
        assert_eq!(
            &line.raw[rows[0].raw_range.start..rows[2].raw_range.end],
            "abcdefghijklm"
        );
    }

    #[test]
    fn wrap_wide_chars() {
        // Each CJK char is 2 columns wide
        let line = Line::new(0, "あいうえお".into());
        // width=6: "あいう" (6 cols), "えお" (4 cols)
        assert_eq!(line.row_count(6), 2);
        assert_eq!(wrap_row_text(&line, 6, 0), "あいう");
        assert_eq!(wrap_row_text(&line, 6, 1), "えお");
    }

    #[test]
    fn wrap_wide_char_at_boundary() {
        // width=5: "あい" (4 cols), then "う" (2 cols) won't fit -> wrap
        let line = Line::new(0, "あいう".into());
        assert_eq!(line.row_count(5), 2);
        assert_eq!(wrap_row_text(&line, 5, 0), "あい");
        assert_eq!(wrap_row_text(&line, 5, 1), "う");
    }

    #[test]
    fn empty_line() {
        let line = Line::new(0, String::new());
        assert_eq!(line.row_count(80), 1);
        assert_eq!(wrap_row_text(&line, 80, 0), "");
    }

    // --- ANSI escape sequence tests ---

    #[test]
    fn plain_text_strips_escapes() {
        let line = Line::new(0, "\x1b[1mHi\x1b[0m, 😀".into());
        assert_eq!(line.raw(), "\x1b[1mHi\x1b[0m, 😀");
        // plain should have escapes stripped
        assert_eq!(line.plain, "Hi, 😀");
    }

    #[test]
    fn plain_to_raw_mapping() {
        let line = Line::new(0, "\x1b[1mHi\x1b[0m, 😀".into());
        assert_eq!(
            line.plain_to_raw,
            vec![
                4, 5, // "Hi"
                10, 11, // ", "
                12, 13, 14, 15, // 😀
            ]
        );
    }

    #[test]
    fn wrap_with_escapes_no_wrap_needed() {
        // "\x1b[31m" = 5 bytes, "Hi" = 2 bytes, "\x1b[0m" = 4 bytes
        // Visible: "Hi" = 2 columns
        let line = Line::new(0, "\x1b[31mHi\x1b[0m".into());
        assert_eq!(line.row_count(10), 1);
        assert_eq!(wrap_row_text(&line, 10, 0), "\x1b[31mHi\x1b[0m");
    }

    #[test]
    fn wrap_with_escapes_causes_wrap() {
        // Visible text: "HelloWorld" (10 chars), width=5
        // The reset sequence follows "Hello" in raw text, so it stays in row 0.
        let line = Line::new(0, "\x1b[1mHello\x1b[0mWorld".into());
        assert_eq!(line.row_count(5), 2);
        assert_eq!(wrap_row_text(&line, 5, 0), "\x1b[1mHello\x1b[0m");
        assert_eq!(wrap_row_text(&line, 5, 1), "World");
        let rows = line.wrap(5);
        assert_eq!(
            &line.raw[rows[0].raw_range.start..rows[1].raw_range.end],
            "\x1b[1mHello\x1b[0mWorld"
        );
    }

    #[test]
    fn wrap_escape_at_wrap_boundary() {
        // Visible: "abcde12345" (10 chars), width=5
        // Escape between "abcde" and "12345" stays in row 0.
        let line = Line::new(0, "abcde\x1b[31m12345".into());
        assert_eq!(line.row_count(5), 2);
        assert_eq!(wrap_row_text(&line, 5, 0), "abcde\x1b[31m");
        assert_eq!(wrap_row_text(&line, 5, 1), "12345");
    }

    #[test]
    fn wrap_multiple_escapes() {
        // Visible: "abcdefghij" (10 chars), width=5
        // The escape preceding 'f' lands in row 0 since it's before the wrap point.
        let line = Line::new(
            0,
            "\x1b[1ma\x1b[2mb\x1b[3mc\x1b[4md\x1b[5me\x1b[6mfghij".into(),
        );
        assert_eq!(line.row_count(5), 2);
        assert_eq!(
            wrap_row_text(&line, 5, 0),
            "\x1b[1ma\x1b[2mb\x1b[3mc\x1b[4md\x1b[5me\x1b[6m"
        );
        assert_eq!(wrap_row_text(&line, 5, 1), "fghij");
    }

    #[test]
    fn escape_only_line() {
        // Line with only escape sequences, no visible text
        let line = Line::new(0, "\x1b[0m\x1b[1m".into());
        assert_eq!(line.row_count(80), 1);
        assert_eq!(wrap_row_text(&line, 80, 0), "\x1b[0m\x1b[1m");
    }

    #[test]
    fn wrap_wide_chars_with_escapes() {
        // Visible: "あいう" (6 cols), width=5 -> wraps after "あい" (4 cols)
        let line = Line::new(0, "\x1b[31mあいう\x1b[0m".into());
        assert_eq!(line.row_count(5), 2);
        assert_eq!(wrap_row_text(&line, 5, 0), "\x1b[31mあい");
        assert_eq!(wrap_row_text(&line, 5, 1), "う\x1b[0m");
    }

    // --- Match search tests ---

    fn match_pos(line_index: usize, range: Range<usize>) -> MatchPosition {
        MatchPosition {
            line_index,
            plain_range: range,
        }
    }

    fn collect_from(line: &Line, query: &Regex, from: SearchLineFrom) -> Vec<MatchPosition> {
        line.matches_iter(query, from).collect()
    }

    #[test]
    fn matches_iter_start_returns_all_matches() {
        let line = Line::new(0, "ab cd ab ef ab".into());
        let query = Regex::new("ab").unwrap();
        assert_eq!(
            collect_from(&line, &query, SearchLineFrom::Start),
            vec![match_pos(0, 0..2), match_pos(0, 6..8), match_pos(0, 12..14),]
        );
    }

    #[test]
    fn matches_iter_start_no_match() {
        let line = Line::new(0, "hello world".into());
        let query = Regex::new("xyz").unwrap();
        assert!(collect_from(&line, &query, SearchLineFrom::Start).is_empty());
    }

    #[test]
    fn matches_iter_next_of_excludes_border_and_earlier() {
        let line = Line::new(0, "ab cd ab ef ab".into());
        let query = Regex::new("ab").unwrap();
        let border = match_pos(0, 6..8); // The middle "ab".
        assert_eq!(
            collect_from(&line, &query, SearchLineFrom::NextOf(&border)),
            vec![match_pos(0, 12..14)]
        );
    }

    #[test]
    fn matches_iter_next_of_at_match_start_includes_self() {
        // border ends exactly where another match starts. The starting match is included
        // because the predicate uses `m.start < border.end`, not `<=`.
        let line = Line::new(0, "abab".into());
        let query = Regex::new("ab").unwrap();
        let border = match_pos(0, 0..2);
        assert_eq!(
            collect_from(&line, &query, SearchLineFrom::NextOf(&border)),
            vec![match_pos(0, 2..4)]
        );
    }

    #[test]
    fn matches_iter_prev_of_excludes_border_and_later() {
        let line = Line::new(0, "ab cd ab ef ab".into());
        let query = Regex::new("ab").unwrap();
        let border = match_pos(0, 6..8);
        assert_eq!(
            collect_from(&line, &query, SearchLineFrom::PrevOf(&border)),
            vec![match_pos(0, 0..2)]
        );
    }

    #[test]
    fn matches_iter_prev_of_at_match_end_includes_self() {
        // border starts exactly where the previous match ends. Predicate uses
        // `m.end <= border.start`, so the prior match is kept.
        let line = Line::new(0, "abab".into());
        let query = Regex::new("ab").unwrap();
        let border = match_pos(0, 2..4);
        assert_eq!(
            collect_from(&line, &query, SearchLineFrom::PrevOf(&border)),
            vec![match_pos(0, 0..2)]
        );
    }

    #[test]
    fn matches_iter_row_skips_matches_before_row_start() {
        // Wrap "abcdefghij" at width 5: row 0 = "abcde", row 1 = "fghij".
        let line = Line::new(0, "abcdefghij".into());
        let query = Regex::new("[a-j]{2}").unwrap();
        let rows = line.wrap(5);
        // Row 0 starts at raw 0, so all matches are kept.
        assert_eq!(
            collect_from(&line, &query, SearchLineFrom::Row(&rows[0])),
            vec![
                match_pos(0, 0..2),
                match_pos(0, 2..4),
                match_pos(0, 4..6),
                match_pos(0, 6..8),
                match_pos(0, 8..10),
            ]
        );
        // Row 1 starts at raw 5, so matches whose start maps to raw < 5 are skipped.
        assert_eq!(
            collect_from(&line, &query, SearchLineFrom::Row(&rows[1])),
            vec![match_pos(0, 6..8), match_pos(0, 8..10)]
        );
    }

    #[test]
    fn matches_iter_row_with_escapes_uses_raw_position() {
        // raw: "\x1b[1mabcde\x1b[0mfghij" — wrap at width 5.
        // row 0 raw_range covers "\x1b[1mabcde\x1b[0m" (raw 0..13),
        // row 1 starts at raw 13 ("fghij").
        let line = Line::new(0, "\x1b[1mabcde\x1b[0mfghij".into());
        let query = Regex::new("[a-j]{2}").unwrap();
        let rows = line.wrap(5);
        assert_eq!(rows[1].raw_range.start, 13);
        // For row 1: matches whose plain start maps to raw < 13 are skipped.
        // plain_range 0..2 → raw 4..6 (skipped), 4..6 → raw 8..13 (skipped at start=8),
        // 6..8 → raw 13..15 (kept), 8..10 → raw 15..17 (kept).
        assert_eq!(
            collect_from(&line, &query, SearchLineFrom::Row(&rows[1])),
            vec![match_pos(0, 6..8), match_pos(0, 8..10)]
        );
    }

    #[test]
    fn matches_iter_row_handles_zero_width_match_at_end() {
        // Regression: a zero-width match at plain.len() must not panic on
        // plain_to_raw indexing.
        let line = Line::new(0, "abc".into());
        let query = Regex::new("$").unwrap();
        let rows = line.wrap(80);
        let result = collect_from(&line, &query, SearchLineFrom::Row(&rows[0]));
        assert_eq!(result, vec![match_pos(0, 3..3)]);
    }

    #[test]
    fn find_first_match_from_returns_first() {
        let line = Line::new(0, "ab cd ab ef ab".into());
        let query = Regex::new("ab").unwrap();
        let border = match_pos(0, 0..2);
        assert_eq!(
            line.find_first_match_from(&query, SearchLineFrom::NextOf(&border)),
            Some(match_pos(0, 6..8))
        );
        assert_eq!(
            line.find_first_match_from(&Regex::new("xyz").unwrap(), SearchLineFrom::Start),
            None
        );
    }

    #[test]
    fn find_last_match_from_returns_last() {
        let line = Line::new(0, "ab cd ab ef ab".into());
        let query = Regex::new("ab").unwrap();
        let border = match_pos(0, 12..14);
        assert_eq!(
            line.find_last_match_from(&query, SearchLineFrom::PrevOf(&border)),
            Some(match_pos(0, 6..8))
        );
        assert_eq!(
            line.find_last_match_from(&Regex::new("xyz").unwrap(), SearchLineFrom::Start),
            None
        );
    }
}
