/// Search match highlighting for lines containing ANSI escape sequences.
///
/// Converts match positions from plain text coordinates to raw text coordinates,
/// then injects reverse-video escape sequences to highlight matches while
/// preserving existing styling.
use std::ops::Range;

/// Escape sequences for reverse video (highlight on/off).
const REVERSE_ON: &str = "\x1b[7m";
const REVERSE_OFF: &str = "\x1b[27m";

/// Positions in raw text where highlight styling should be injected.
#[derive(Debug)]
pub struct HighlightPositions {
    positions: Vec<HighlightPos>,
}

/// A single highlight injection point in the raw text.
#[derive(Debug)]
struct HighlightPos {
    /// Byte position in the raw text.
    index: usize,
    kind: HighlightPosKind,
}

#[derive(Debug)]
enum HighlightPosKind {
    /// Match start: turn reverse video on.
    Start,
    /// After an escape sequence inside a match: re-apply reverse video.
    InnerControlEnd,
    /// Match end: turn reverse video off.
    End,
}

/// Build highlight positions from plain-text match ranges.
///
/// Uses `plain_to_raw` mapping to convert plain-text byte ranges to raw-text
/// positions. Detects escape sequences within matches by checking for gaps in
/// the mapping and inserts `InnerControlEnd` markers so highlighting is
/// re-applied after each internal escape sequence.
pub fn build_highlight_positions(
    plain_ranges: &[(usize, usize)],
    plain_to_raw: &[usize],
    raw_len: usize,
) -> HighlightPositions {
    let mut positions = Vec::new();

    for &(start, end) in plain_ranges {
        if start >= end {
            continue;
        }

        let i_raw = plain_to_raw[start];
        positions.push(HighlightPos {
            index: i_raw,
            kind: HighlightPosKind::Start,
        });

        // Detect escape sequences within the match range by looking for gaps
        // in the plain_to_raw mapping.
        let mut prev_i_p = start;
        for i_p in start..end {
            let i_raw = plain_to_raw[i_p];
            let prev_i_raw = plain_to_raw[prev_i_p];
            if i_raw - prev_i_raw > 1 {
                positions.push(HighlightPos {
                    index: i_raw,
                    kind: HighlightPosKind::InnerControlEnd,
                });
            }
            prev_i_p = i_p;
        }

        let end_raw = if end < plain_to_raw.len() {
            plain_to_raw[end]
        } else {
            raw_len
        };
        positions.push(HighlightPos {
            index: end_raw,
            kind: HighlightPosKind::End,
        });
    }

    HighlightPositions { positions }
}

/// Apply highlight escape sequences to a range of raw text.
///
/// `raw_range` specifies which portion of the full raw text to render
/// (e.g., a single wrapped row). Only highlight positions within that range
/// are processed. Returns the text with reverse-video escapes injected.
pub fn apply_highlight_to_range(
    raw_text: &str,
    raw_range: Range<usize>,
    positions: &HighlightPositions,
) -> String {
    let slice = &raw_text[raw_range.clone()];

    // Find positions relevant to this range and track if we start inside a highlight.
    let mut i_pos_from = 0;
    let mut range_start_in_highlight = false;
    for (i, pos) in positions.positions.iter().enumerate() {
        if raw_range.start <= pos.index {
            i_pos_from = i;
            break;
        }
        range_start_in_highlight = !matches!(pos.kind, HighlightPosKind::End);
    }

    let mut result = String::with_capacity(slice.len() + 32);

    if range_start_in_highlight {
        result.push_str(REVERSE_ON);
    }

    let mut i_prev = raw_range.start;
    let mut range_end_in_highlight = false;

    for pos in positions.positions.iter().skip(i_pos_from) {
        if raw_range.end <= pos.index {
            break;
        }
        result.push_str(&raw_text[i_prev..pos.index]);
        i_prev = pos.index;
        match pos.kind {
            HighlightPosKind::Start | HighlightPosKind::InnerControlEnd => {
                result.push_str(REVERSE_ON);
                range_end_in_highlight = true;
            }
            HighlightPosKind::End => {
                result.push_str(REVERSE_OFF);
                range_end_in_highlight = false;
            }
        }
    }

    result.push_str(&raw_text[i_prev..raw_range.end]);

    if range_end_in_highlight {
        result.push_str(REVERSE_OFF);
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::line::Line;

    /// Helper to build positions from a line and plain-text ranges.
    fn build_from_line(line: &Line, ranges: &[(usize, usize)]) -> HighlightPositions {
        build_highlight_positions(ranges, line.plain_to_raw(), line.raw().len())
    }

    /// Helper to apply highlight to the full raw text.
    fn apply_full(line: &Line, positions: &HighlightPositions) -> String {
        apply_highlight_to_range(line.raw(), 0..line.raw().len(), positions)
    }

    #[test]
    fn plain_text_single_match() {
        let line = Line::new("hello world".into());
        let positions = build_from_line(&line, &[(6, 11)]); // "world"
        let result = apply_full(&line, &positions);
        assert_eq!(result, "hello \x1b[7mworld\x1b[27m");
    }

    #[test]
    fn plain_text_multiple_matches() {
        let line = Line::new("foo bar foo".into());
        let positions = build_from_line(&line, &[(0, 3), (8, 11)]); // both "foo"
        let result = apply_full(&line, &positions);
        assert_eq!(result, "\x1b[7mfoo\x1b[27m bar \x1b[7mfoo\x1b[27m");
    }

    #[test]
    fn match_spanning_escape_sequence() {
        // raw:   "This is \x1b[1mCargo\x1b[0m.toml"
        // plain: "This is Cargo.toml"
        // match: "go.to" -> plain 11..16
        let line = Line::new("This is \x1b[1mCargo\x1b[0m.toml".into());
        let positions = build_from_line(&line, &[(11, 16)]);
        let result = apply_full(&line, &positions);
        // After "go", there's \x1b[0m, then ".to" starts. InnerControlEnd re-applies reverse.
        assert_eq!(
            result,
            "This is \x1b[1mCar\x1b[7mgo\x1b[0m\x1b[7m.to\x1b[27mml"
        );
    }

    #[test]
    fn no_matches() {
        let line = Line::new("hello world".into());
        let positions = build_from_line(&line, &[]);
        let result = apply_full(&line, &positions);
        assert_eq!(result, "hello world");
    }

    #[test]
    fn match_at_beginning() {
        let line = Line::new("hello world".into());
        let positions = build_from_line(&line, &[(0, 5)]); // "hello"
        let result = apply_full(&line, &positions);
        assert_eq!(result, "\x1b[7mhello\x1b[27m world");
    }

    #[test]
    fn match_entire_line() {
        let line = Line::new("abc".into());
        let positions = build_from_line(&line, &[(0, 3)]);
        let result = apply_full(&line, &positions);
        assert_eq!(result, "\x1b[7mabc\x1b[27m");
    }

    #[test]
    fn highlight_applied_to_wrapped_row_range() {
        // raw:   "abcdefghij" (no escapes)
        // plain: "abcdefghij"
        // match: "ef" -> plain 4..6
        // At width 5: row 0 = raw 0..5 ("abcde"), row 1 = raw 5..10 ("fghij")
        let line = Line::new("abcdefghij".into());
        let positions = build_from_line(&line, &[(4, 6)]);

        // Row 0: raw 0..5 -> "abcde" with "e" highlighted
        let r0 = apply_highlight_to_range(line.raw(), 0..5, &positions);
        assert_eq!(r0, "abcd\x1b[7me\x1b[27m");

        // Row 1: raw 5..10 -> "fghij" with "f" highlighted
        let r1 = apply_highlight_to_range(line.raw(), 5..10, &positions);
        assert_eq!(r1, "\x1b[7mf\x1b[27mghij");
    }
}
