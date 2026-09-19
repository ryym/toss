use unicode_width::UnicodeWidthChar;

use crate::document::Document;
use crate::line::Row;
use crate::line_editor::InputAtCursor;
use crate::pager::PagerMode;

/// ANSI reverse-video on/off, used to render the view-mode status line like `less`.
pub(super) const STATUS_REVERSE_ON: &str = "\x1b[7m";
pub(super) const STATUS_REVERSE_OFF: &str = "\x1b[27m";

/// Build the status line for the current mode.
pub(super) fn build(mode: &PagerMode, rows: &[Row], width: usize, doc: &Document) -> String {
    match mode {
        PagerMode::View => {
            let line = clip(&position(rows, doc), width);
            format!("{STATUS_REVERSE_ON}{line}{STATUS_REVERSE_OFF}")
        }
        PagerMode::SearchInput(search) => {
            let input = search.editor.at_cursor();
            search_input(search.direction.prompt(), input, width)
        }
    }
}

/// Build the search prompt line, e.g. `/query`, with the cell the cursor covers shown in
/// reverse video. The line is clipped to `width` columns, keeping the right side.
fn search_input(prompt: &str, input: InputAtCursor, width: usize) -> String {
    let (cell, tail) = cursor_cell(input.from_cursor);
    let head: String = prompt.chars().chain(input.before.iter().copied()).collect();

    let cursor_and_tail_width = text_width(&cell) + text_width(&tail);
    if cursor_and_tail_width > width {
        return clip(&format!("{cell}{tail}"), width);
    }

    let head = clip(&head, width - cursor_and_tail_width);
    format!("{head}{STATUS_REVERSE_ON}{cell}{STATUS_REVERSE_OFF}{tail}")
}

/// Split `from_cursor`, the input from the cursor on, into the cell the cursor covers and
/// the rest. The cell is one character plus any zero-width characters that follow it, since
/// a combining mark renders on the cell of the character it follows rather than on one of
/// its own.
fn cursor_cell(from_cursor: &[char]) -> (String, String) {
    let Some((ch, rest)) = from_cursor.split_first() else {
        // The cursor sits past the last character, where it covers a space of its own.
        return (" ".to_string(), String::new());
    };
    let marks = rest.iter().take_while(|ch| char_width(**ch) == 0).count();
    let cell = std::iter::once(*ch)
        .chain(rest[..marks].iter().copied())
        .collect();
    (cell, rest[marks..].iter().collect())
}

/// Build the `less`-style position indicator, e.g. `src/pager.rs lines 1-31/1084 2%`.
/// The leading name is omitted for sources without one (stdin). While input is still
/// streaming in, the total is not final, so it is shown as `<count>+` with the
/// percentage omitted. If the input ended with a read error, `[read error]` is shown
/// in place of the percentage to flag that the content is truncated. The range covers
/// the whole viewport, ignoring header/heading overlays.
fn position(rows: &[Row], doc: &Document) -> String {
    let (top, bottom) = match (rows.first(), rows.last()) {
        (Some(first), Some(last)) => (first.line_index() + 1, last.line_index() + 1),
        _ => (0, 0),
    };
    let total = doc.line_count();

    let prefix = match doc.name() {
        Some(name) => format!("{name} "),
        None => String::new(),
    };

    if doc.stream_error().is_some() {
        // The input ended abnormally: the shown lines are a truncation, not the
        // whole input, so flag it instead of a (misleading) final percentage.
        format!("{prefix}lines {top}-{bottom}/{total} [read error]")
    } else if doc.is_complete() {
        let percent = (bottom * 100).checked_div(total).unwrap_or(0);
        format!("{prefix}lines {top}-{bottom}/{total} {percent}%")
    } else {
        // The total is still a growing lower bound; mark it and omit the percentage.
        format!("{prefix}lines {top}-{bottom}/{total}+")
    }
}

/// Clip the status line to `width` display columns, keeping the right side.
/// The most useful information (the position and percentage) sits on the right,
/// so when the line is too long we drop characters from the left instead.
/// A wide character that would straddle the left edge is dropped whole, which may
/// leave the result one column narrower than `width`.
fn clip(line: &str, width: usize) -> String {
    if text_width(line) <= width {
        return line.to_string();
    }
    let mut kept = 0;
    let mut start = line.len();
    for (i, ch) in line.char_indices().rev() {
        let w = char_width(ch);
        if kept + w > width {
            break;
        }
        kept += w;
        start = i;
    }
    line[start..].to_string()
}

fn text_width(text: &str) -> usize {
    text.chars().map(char_width).sum()
}

fn char_width(ch: char) -> usize {
    ch.width().unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clip_keeps_right_side() {
        // Fits: returned unchanged.
        assert_eq!(clip("lines 3-30/500 2%", 20), "lines 3-30/500 2%");
        // Too long: drop from the left, keep the rightmost columns.
        assert_eq!(clip("lines 3-30/500 2%", 14), "es 3-30/500 2%");
        assert_eq!(clip("lines 3-30/500 2%", 4), "0 2%");
    }

    /// Build a search prompt line, with the reversed span shown as `[...]` instead of escape
    /// sequences. The cursor sits on the first character of `from_cursor`.
    fn prompt(before: &str, from_cursor: &str, width: usize) -> String {
        let before: Vec<char> = before.chars().collect();
        let from_cursor: Vec<char> = from_cursor.chars().collect();
        let input = InputAtCursor {
            before: &before,
            from_cursor: &from_cursor,
        };
        search_input("/", input, width)
            .replace(STATUS_REVERSE_ON, "[")
            .replace(STATUS_REVERSE_OFF, "]")
    }

    #[test]
    fn cursor_reverses_the_cell_it_sits_on() {
        assert_eq!(prompt("a", "bc", 10), "/a[b]c");
        // Past the last character, the cursor gets a space of its own.
        assert_eq!(prompt("abc", "", 10), "/abc[ ]");
    }

    #[test]
    fn cursor_reverses_a_wide_char_whole() {
        assert_eq!(prompt("", "あx", 10), "/[あ]x");
    }

    #[test]
    fn cursor_keeps_combining_marks_in_its_cell() {
        // "a" and the combining acute that follows it render as one cell.
        assert_eq!(prompt("", "a\u{301}bc", 10), "/[a\u{301}]bc");
        // The mark takes no column of its own, so it must not be mistaken for the cursor cell.
        assert_eq!(prompt("a\u{301}", "bc", 10), "/a\u{301}[b]c");
    }

    #[test]
    fn clipping_drops_the_left_side_to_keep_the_cursor() {
        // The cursor and the rest fit, so only what precedes them is clipped.
        assert_eq!(prompt("ab", "cde", 6), "/ab[c]de");
        assert_eq!(prompt("ab", "cde", 5), "ab[c]de");
    }

    #[test]
    fn cursor_is_unmarked_once_it_no_longer_fits() {
        // The cursor cell and what follows are wider than the screen on their own.
        assert_eq!(prompt("a", "bcde", 3), "cde");
    }

    #[test]
    fn clip_drops_straddling_wide_char() {
        // "あ" is 2 columns wide. With width 1 or 2, only "x" (right side) fits and
        // the wide char is dropped whole; width 3 fits both.
        assert_eq!(clip("あx", 1), "x");
        assert_eq!(clip("あx", 2), "x");
        assert_eq!(clip("あx", 3), "あx");
    }
}
