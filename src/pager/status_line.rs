use unicode_width::UnicodeWidthChar;

use crate::document::Document;
use crate::pager::{PagerMode, viewport::Viewport};

/// ANSI reverse-video on/off, used to render the view-mode status line like `less`.
pub(super) const STATUS_REVERSE_ON: &str = "\x1b[7m";
pub(super) const STATUS_REVERSE_OFF: &str = "\x1b[27m";

/// Build the status line for the current mode.
pub(super) fn build(mode: &PagerMode, viewport: &Viewport, doc: &Document) -> String {
    let width = viewport.size().width();
    match mode {
        PagerMode::View => {
            let line = clip(&position(viewport, doc), width);
            format!("{STATUS_REVERSE_ON}{line}{STATUS_REVERSE_OFF}")
        }
        PagerMode::SearchInput(search) => {
            let line = format!(
                "{}{}",
                search.direction.prompt(),
                search.editor.input_with_cursor()
            );
            clip(&line, width)
        }
    }
}

/// Build the `less`-style position indicator, e.g. `src/pager.rs lines 1-31/1084 2%`.
/// The leading name is omitted for sources without one (stdin). While input is still
/// streaming in, the total is not final, so it is shown as `<count>+` with the
/// percentage omitted. If the input ended with a read error, `[read error]` is shown
/// in place of the percentage to flag that the content is truncated. The range covers
/// the whole viewport, ignoring header/heading overlays.
fn position(viewport: &Viewport, doc: &Document) -> String {
    let rows = viewport.rows();
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
    let total: usize = line.chars().map(|c| c.width().unwrap_or(0)).sum();
    if total <= width {
        return line.to_string();
    }
    let mut kept = 0;
    let mut start = line.len();
    for (i, ch) in line.char_indices().rev() {
        let w = ch.width().unwrap_or(0);
        if kept + w > width {
            break;
        }
        kept += w;
        start = i;
    }
    line[start..].to_string()
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

    #[test]
    fn clip_drops_straddling_wide_char() {
        // "あ" is 2 columns wide. With width 1 or 2, only "x" (right side) fits and
        // the wide char is dropped whole; width 3 fits both.
        assert_eq!(clip("あx", 1), "x");
        assert_eq!(clip("あx", 2), "x");
        assert_eq!(clip("あx", 3), "あx");
    }
}
