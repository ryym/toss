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

use std::ops::Range;

use regex::Regex;
use unicode_width::UnicodeWidthChar;

use crate::ansi;

/// A single line of text from the document.
#[derive(Debug, Clone)]
pub struct Line {
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
    pub fn new(raw_line: String) -> Self {
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
            raw: raw_line,
            plain,
            plain_to_raw,
        }
    }

    /// Returns the original raw text including ANSI escape sequences.
    pub fn raw(&self) -> &str {
        &self.raw
    }

    /// Returns the byte index mapping from plain text to raw text.
    pub fn plain_to_raw(&self) -> &[usize] {
        &self.plain_to_raw
    }

    /// Returns the plain text without ANSI escape sequences.
    pub fn plain(&self) -> &str {
        &self.plain
    }

    /// Returns the original raw text including ANSI escape sequences.
    #[cfg(test)]
    pub fn text(&self) -> &str {
        &self.raw
    }

    /// Compute the raw byte ranges for each wrapped row at the given width.
    ///
    /// Wrapping is computed on the plain text (visible characters only) but
    /// ranges refer to the raw text so that slicing produces correct output
    /// including escape sequences.
    pub fn wrap(&self, width: usize) -> Vec<Range<usize>> {
        if width == 0 {
            let all = 0..self.raw.len();
            return vec![all];
        }

        let mut ranges = Vec::new();
        let mut row_start = 0;
        let mut col = 0;
        let mut i_plain_byte = 0;

        for ch in self.plain.chars() {
            let i_raw = self.plain_to_raw[i_plain_byte];
            let ch_width = ch.width().unwrap_or(0);
            if col + ch_width > width && col > 0 {
                ranges.push(row_start..i_raw);
                row_start = i_raw;
                col = ch_width;
            } else {
                col += ch_width;
            }
            i_plain_byte += ch.len_utf8();
        }
        ranges.push(row_start..self.raw.len());

        ranges
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
    /// Returns a list of (start, end) byte ranges in the plain text.
    pub fn find_matches(&self, query: &Regex) -> Vec<(usize, usize)> {
        query
            .find_iter(&self.plain)
            .map(|m| (m.start(), m.end()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wrap_row_text(line: &Line, width: usize, wrap_index: usize) -> &str {
        let ranges = line.wrap(width);
        &line.raw[ranges[wrap_index].clone()]
    }

    #[test]
    fn wrap_short_line() {
        let line = Line::new("hello".into());
        assert_eq!(line.row_count(80), 1);
        assert_eq!(wrap_row_text(&line, 80, 0), "hello");
    }

    #[test]
    fn wrap_exact_width() {
        let line = Line::new("abcde".into());
        assert_eq!(line.row_count(5), 1);
        assert_eq!(wrap_row_text(&line, 5, 0), "abcde");
    }

    #[test]
    fn wrap_overflow() {
        let line = Line::new("abcdefgh".into());
        assert_eq!(line.row_count(5), 2);
        assert_eq!(wrap_row_text(&line, 5, 0), "abcde");
        assert_eq!(wrap_row_text(&line, 5, 1), "fgh");
    }

    #[test]
    fn wrap_multiple_rows() {
        let line = Line::new("abcdefghijklm".into());
        assert_eq!(line.row_count(5), 3);
        assert_eq!(wrap_row_text(&line, 5, 0), "abcde");
        assert_eq!(wrap_row_text(&line, 5, 1), "fghij");
        assert_eq!(wrap_row_text(&line, 5, 2), "klm");
        // Multi-row range covers consecutive rows
        let ranges = line.wrap(5);
        assert_eq!(&line.raw[ranges[0].start..ranges[1].end], "abcdefghij");
        assert_eq!(&line.raw[ranges[1].start..ranges[2].end], "fghijklm");
        assert_eq!(&line.raw[ranges[0].start..ranges[2].end], "abcdefghijklm");
    }

    #[test]
    fn wrap_wide_chars() {
        // Each CJK char is 2 columns wide
        let line = Line::new("あいうえお".into());
        // width=6: "あいう" (6 cols), "えお" (4 cols)
        assert_eq!(line.row_count(6), 2);
        assert_eq!(wrap_row_text(&line, 6, 0), "あいう");
        assert_eq!(wrap_row_text(&line, 6, 1), "えお");
    }

    #[test]
    fn wrap_wide_char_at_boundary() {
        // width=5: "あい" (4 cols), then "う" (2 cols) won't fit -> wrap
        let line = Line::new("あいう".into());
        assert_eq!(line.row_count(5), 2);
        assert_eq!(wrap_row_text(&line, 5, 0), "あい");
        assert_eq!(wrap_row_text(&line, 5, 1), "う");
    }

    #[test]
    fn empty_line() {
        let line = Line::new(String::new());
        assert_eq!(line.row_count(80), 1);
        assert_eq!(wrap_row_text(&line, 80, 0), "");
    }

    // --- ANSI escape sequence tests ---

    #[test]
    fn plain_text_strips_escapes() {
        let line = Line::new("\x1b[1mHi\x1b[0m, 😀".into());
        assert_eq!(line.text(), "\x1b[1mHi\x1b[0m, 😀");
        // plain should have escapes stripped
        assert_eq!(line.plain, "Hi, 😀");
    }

    #[test]
    fn plain_to_raw_mapping() {
        let line = Line::new("\x1b[1mHi\x1b[0m, 😀".into());
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
        let line = Line::new("\x1b[31mHi\x1b[0m".into());
        assert_eq!(line.row_count(10), 1);
        assert_eq!(wrap_row_text(&line, 10, 0), "\x1b[31mHi\x1b[0m");
    }

    #[test]
    fn wrap_with_escapes_causes_wrap() {
        // Visible text: "HelloWorld" (10 chars), width=5
        // The reset sequence follows "Hello" in raw text, so it stays in row 0.
        let line = Line::new("\x1b[1mHello\x1b[0mWorld".into());
        assert_eq!(line.row_count(5), 2);
        assert_eq!(wrap_row_text(&line, 5, 0), "\x1b[1mHello\x1b[0m");
        assert_eq!(wrap_row_text(&line, 5, 1), "World");
        let ranges = line.wrap(5);
        assert_eq!(
            &line.raw[ranges[0].start..ranges[1].end],
            "\x1b[1mHello\x1b[0mWorld"
        );
    }

    #[test]
    fn wrap_escape_at_wrap_boundary() {
        // Visible: "abcde12345" (10 chars), width=5
        // Escape between "abcde" and "12345" stays in row 0.
        let line = Line::new("abcde\x1b[31m12345".into());
        assert_eq!(line.row_count(5), 2);
        assert_eq!(wrap_row_text(&line, 5, 0), "abcde\x1b[31m");
        assert_eq!(wrap_row_text(&line, 5, 1), "12345");
    }

    #[test]
    fn wrap_multiple_escapes() {
        // Visible: "abcdefghij" (10 chars), width=5
        // The escape preceding 'f' lands in row 0 since it's before the wrap point.
        let line = Line::new("\x1b[1ma\x1b[2mb\x1b[3mc\x1b[4md\x1b[5me\x1b[6mfghij".into());
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
        let line = Line::new("\x1b[0m\x1b[1m".into());
        assert_eq!(line.row_count(80), 1);
        assert_eq!(wrap_row_text(&line, 80, 0), "\x1b[0m\x1b[1m");
    }

    #[test]
    fn wrap_wide_chars_with_escapes() {
        // Visible: "あいう" (6 cols), width=5 -> wraps after "あい" (4 cols)
        let line = Line::new("\x1b[31mあいう\x1b[0m".into());
        assert_eq!(line.row_count(5), 2);
        assert_eq!(wrap_row_text(&line, 5, 0), "\x1b[31mあい");
        assert_eq!(wrap_row_text(&line, 5, 1), "う\x1b[0m");
    }
}
