//! Search match highlighting for lines containing ANSI escape sequences.
//!
//! Converts match positions from plain text coordinates to raw text coordinates,
//! then injects reverse-video escape sequences to highlight matches while
//! preserving existing styling.

use std::{borrow::Cow, ops::Range};

use crate::{
    line::{Line, MatchPosition},
    search::SearchState,
};

/// Highlight style for search matches.
#[derive(Debug, Clone, Copy)]
pub enum HighlightStyle {
    /// used for the current match.
    Reverse,
    /// used for the first character of a non-current match.
    ReverseUnderlineBold,
    /// used for the rest of a non-current match.
    UnderlineBold,
}

impl HighlightStyle {
    fn on_seq(self) -> &'static str {
        match self {
            HighlightStyle::Reverse => "\x1b[7m\x1b[1m",
            HighlightStyle::ReverseUnderlineBold => "\x1b[7m\x1b[4m\x1b[1m",
            HighlightStyle::UnderlineBold => "\x1b[4m\x1b[1m",
        }
    }

    fn off_seq(self) -> &'static str {
        match self {
            HighlightStyle::Reverse => "\x1b[27m\x1b[22m",
            HighlightStyle::ReverseUnderlineBold => "\x1b[27m\x1b[24m\x1b[22m",
            HighlightStyle::UnderlineBold => "\x1b[24m\x1b[22m",
        }
    }
}

/// A single highlight injection point in the raw text.
#[derive(Debug)]
struct HighlightPos {
    /// Byte position in the raw text.
    index: usize,
    kind: HighlightPosKind,
    style: HighlightStyle,
}

#[derive(Debug)]
enum HighlightPosKind {
    /// Match start: turn highlight on.
    Start,
    /// After an escape sequence inside a match: re-apply highlight.
    InnerControlEnd,
    /// Match end: turn highlight off.
    End,
}

/// Highlight parts in a range of `line` that match the search query.
/// Builds a string only if there are matches. Otherwise returns a reference to the `line` range.
pub fn apply_highlight_if_matches<'line>(
    search: Option<&SearchState>,
    line: &'line Line,
    raw_range: Range<usize>,
) -> Cow<'line, str> {
    let search = match search {
        Some(search) => search,
        None => return Cow::Borrowed(&line.raw()[raw_range]),
    };
    let matches = line.find_matches(&search.query);
    if matches.is_empty() {
        return Cow::Borrowed(&line.raw()[raw_range]);
    }
    let positions = build_highlight_positions(&matches, &search.current, line);
    let text = apply_highlight_to_range(line.raw(), raw_range, &positions);
    Cow::Owned(text)
}

/// Build highlight positions from plain-text match ranges.
///
/// The current match (specified by `current_match`) is highlighted with plain
/// reverse video. Each non-current match is split into two styled segments: its
/// first character (reverse + underline + bold) and the rest (underline + bold),
/// so that the start of every match stands out. A single-character non-current
/// match has only the first segment.
///
/// `InnerControlEnd` markers are inserted at every embedded escape sequence
/// inside a segment so that the highlight is re-applied after each one.
fn build_highlight_positions(
    matches: &[MatchPosition],
    current_match: &Option<MatchPosition>,
    line: &Line,
) -> Vec<HighlightPos> {
    let mut positions = Vec::new();

    for m in matches.iter() {
        let raw_range = line.match_raw_range(m);
        let inner: Vec<usize> = line.match_inner_escape_boundaries(m).collect();
        let is_current = current_match.as_ref().is_some_and(|current| current == m);

        if is_current {
            push_segment(&mut positions, HighlightStyle::Reverse, raw_range, &inner);
            continue;
        }

        // Non-current match: reverse the first character, underline + bold the rest.
        let head_end = line.match_first_char_raw_end(m);
        if head_end >= raw_range.end {
            push_segment(
                &mut positions,
                HighlightStyle::ReverseUnderlineBold,
                raw_range,
                &inner,
            );
        } else {
            push_segment(
                &mut positions,
                HighlightStyle::ReverseUnderlineBold,
                raw_range.start..head_end,
                &inner,
            );
            push_segment(
                &mut positions,
                HighlightStyle::UnderlineBold,
                head_end..raw_range.end,
                &inner,
            );
        }
    }

    positions
}

/// Push `Start`/`InnerControlEnd`/`End` positions for one styled segment.
/// Only the embedded escape boundaries strictly inside `raw_range` are emitted;
/// boundaries at a segment edge are handled by the adjacent segment's start/end.
fn push_segment(
    positions: &mut Vec<HighlightPos>,
    style: HighlightStyle,
    raw_range: Range<usize>,
    inner_boundaries: &[usize],
) {
    positions.push(HighlightPos {
        index: raw_range.start,
        kind: HighlightPosKind::Start,
        style,
    });
    for &i_raw in inner_boundaries {
        if raw_range.start < i_raw && i_raw < raw_range.end {
            positions.push(HighlightPos {
                index: i_raw,
                kind: HighlightPosKind::InnerControlEnd,
                style,
            });
        }
    }
    positions.push(HighlightPos {
        index: raw_range.end,
        kind: HighlightPosKind::End,
        style,
    });
}

/// Apply highlight escape sequences to a range of raw text.
///
/// `raw_range` specifies which portion of the full raw text to render
/// (e.g., a single wrapped row). Only highlight positions within that range
/// are processed. Returns the text with reverse-video escapes injected.
fn apply_highlight_to_range(
    raw_text: &str,
    raw_range: Range<usize>,
    positions: &[HighlightPos],
) -> String {
    let slice = &raw_text[raw_range.clone()];

    // Find positions relevant to this range and track if we start inside a highlight.
    let mut i_pos_from = positions.len();
    let mut active_style = None;
    for (i, pos) in positions.iter().enumerate() {
        if raw_range.start <= pos.index {
            i_pos_from = i;
            break;
        }
        active_style = match pos.kind {
            HighlightPosKind::End => None,
            _ => Some(pos.style),
        };
    }

    let mut result = String::with_capacity(slice.len() + 32);

    if let Some(style) = active_style {
        result.push_str(style.on_seq());
    }

    let mut i_prev = raw_range.start;

    for pos in positions.iter().skip(i_pos_from) {
        if raw_range.end <= pos.index {
            break;
        }
        result.push_str(&raw_text[i_prev..pos.index]);
        i_prev = pos.index;
        match pos.kind {
            HighlightPosKind::Start | HighlightPosKind::InnerControlEnd => {
                result.push_str(pos.style.on_seq());
                active_style = Some(pos.style);
            }
            HighlightPosKind::End => {
                result.push_str(pos.style.off_seq());
                active_style = None;
            }
        }
    }

    result.push_str(&raw_text[i_prev..raw_range.end]);

    if let Some(style) = active_style {
        result.push_str(style.off_seq());
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::line::Line;

    /// Helper to build positions from a line and plain-text ranges.
    /// The first range is treated as the current match.
    fn build_from_line(line: &Line, ranges: &[(usize, usize)]) -> Vec<HighlightPos> {
        let matches = ranges
            .iter()
            .map(|r| MatchPosition::new(0, r.0..r.1))
            .collect::<Vec<_>>();
        build_highlight_positions(&matches, &matches.get(0).cloned(), line)
    }

    /// Helper to apply highlight to the full raw text.
    fn apply_full(line: &Line, positions: &[HighlightPos]) -> String {
        apply_highlight_to_range(line.raw(), 0..line.raw().len(), positions)
    }

    #[test]
    fn plain_text_single_match() {
        let line = Line::new(0, "hello world".into());
        let positions = build_from_line(&line, &[(6, 11)]); // "world"
        let result = apply_full(&line, &positions);
        assert_eq!(result, "hello \x1b[7m\x1b[1mworld\x1b[27m\x1b[22m");
    }

    #[test]
    fn plain_text_multiple_matches() {
        let line = Line::new(0, "foo bar foo".into());
        let positions = build_from_line(&line, &[(0, 3), (8, 11)]); // both "foo"
        let result = apply_full(&line, &positions);
        assert_eq!(
            result,
            "\x1b[7m\x1b[1mfoo\x1b[27m\x1b[22m bar \x1b[7m\x1b[4m\x1b[1mf\x1b[27m\x1b[24m\x1b[22m\x1b[4m\x1b[1moo\x1b[24m\x1b[22m"
        );
    }

    #[test]
    fn match_spanning_escape_sequence() {
        // raw:   "This is \x1b[1mCargo\x1b[0m.toml"
        // plain: "This is Cargo.toml"
        // match: "go.to" -> plain 11..16
        let line = Line::new(0, "This is \x1b[1mCargo\x1b[0m.toml".into());
        let positions = build_from_line(&line, &[(11, 16)]);
        let result = apply_full(&line, &positions);
        // After "go", there's \x1b[0m, then ".to" starts. InnerControlEnd re-applies
        // the highlight (reverse + bold).
        assert_eq!(
            result,
            "This is \x1b[1mCar\x1b[7m\x1b[1mgo\x1b[0m\x1b[7m\x1b[1m.to\x1b[27m\x1b[22mml"
        );
    }

    #[test]
    fn no_matches() {
        let line = Line::new(0, "hello world".into());
        let positions = build_from_line(&line, &[]);
        let result = apply_full(&line, &positions);
        assert_eq!(result, "hello world");
    }

    #[test]
    fn match_at_beginning() {
        let line = Line::new(0, "hello world".into());
        let positions = build_from_line(&line, &[(0, 5)]); // "hello"
        let result = apply_full(&line, &positions);
        assert_eq!(result, "\x1b[7m\x1b[1mhello\x1b[27m\x1b[22m world");
    }

    #[test]
    fn match_entire_line() {
        let line = Line::new(0, "abc".into());
        let positions = build_from_line(&line, &[(0, 3)]);
        let result = apply_full(&line, &positions);
        assert_eq!(result, "\x1b[7m\x1b[1mabc\x1b[27m\x1b[22m");
    }

    #[test]
    fn highlight_applied_to_wrapped_row_range() {
        // raw:   "abcdefghij" (no escapes)
        // plain: "abcdefghij"
        // match: "ef" -> plain 4..6
        // At width 5: row 0 = raw 0..5 ("abcde"), row 1 = raw 5..10 ("fghij")
        let line = Line::new(0, "abcdefghij".into());
        let positions = build_from_line(&line, &[(4, 6)]);

        // Row 0: raw 0..5 -> "abcde" with "e" highlighted
        let r0 = apply_highlight_to_range(line.raw(), 0..5, &positions);
        assert_eq!(r0, "abcd\x1b[7m\x1b[1me\x1b[27m\x1b[22m");

        // Row 1: raw 5..10 -> "fghij" with "f" highlighted
        let r1 = apply_highlight_to_range(line.raw(), 5..10, &positions);
        assert_eq!(r1, "\x1b[7m\x1b[1mf\x1b[27m\x1b[22mghij");
    }
}
