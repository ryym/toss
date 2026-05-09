//! Search state and navigation for finding matches in a document.

use std::ops::Range;

use regex::Regex;

use crate::document::Document;

/// Direction of search.
#[derive(Debug, Clone, Copy)]
pub enum SearchDirection {
    Forward,
    Backward,
}

impl SearchDirection {
    /// Returns the prompt character for this direction.
    pub fn prompt(&self) -> &'static str {
        match self {
            SearchDirection::Forward => "/",
            SearchDirection::Backward => "?",
        }
    }

    /// Returns the opposite direction.
    pub fn opposite(&self) -> Self {
        match self {
            SearchDirection::Forward => SearchDirection::Backward,
            SearchDirection::Backward => SearchDirection::Forward,
        }
    }
}

/// Position of a specific match: line index and a text range in the line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchPosition {
    pub line_index: usize,
    /// A match range in the line's plain text.
    pub plain_range: Range<usize>,
}

/// Active search state, preserved across search submissions.
pub struct SearchState {
    pub query: Regex,
    pub direction: SearchDirection,
    /// Current match position (line and which match within the line).
    pub current: Option<MatchPosition>,
}

/// Find the next match from `from_line` in the given direction.
/// Wraps around the document if no match is found before the end/start.
pub fn find_next_match(
    doc: &mut Document,
    query: &Regex,
    from_line: usize,
    direction: SearchDirection,
) -> Option<MatchPosition> {
    let line_count = doc.line_count();
    if line_count == 0 {
        return None;
    }

    match direction {
        SearchDirection::Forward => {
            // Search from_line..end, then 0..from_line (wrap around)
            for i in from_line..line_count {
                if let Some(pos) = first_match(doc, query, i, direction) {
                    return Some(pos);
                }
            }
            for i in 0..from_line {
                if let Some(pos) = first_match(doc, query, i, direction) {
                    return Some(pos);
                }
            }
        }
        SearchDirection::Backward => {
            // Search from_line..0 (reverse), then end..from_line (reverse, wrap around)
            for i in (0..=from_line).rev() {
                if let Some(pos) = first_match(doc, query, i, direction) {
                    return Some(pos);
                }
            }
            for i in (from_line + 1..line_count).rev() {
                if let Some(pos) = first_match(doc, query, i, direction) {
                    return Some(pos);
                }
            }
        }
    }

    None
}

/// Find the next match within the same line after `match_index`.
/// Returns None if there are no more matches on this line in the given direction.
pub fn find_next_match_in_line(
    doc: &mut Document,
    query: &Regex,
    current: &MatchPosition,
    direction: SearchDirection,
) -> Option<(usize, usize)> {
    let line = doc.line(current.line_index)?;
    match direction {
        SearchDirection::Forward => {
            let matches = line.find_matches_within(query, current.plain_range.end..);
            matches.first().copied()
        }
        SearchDirection::Backward => {
            let matches = line.find_matches_within(query, ..current.plain_range.start);
            matches.last().copied()
        }
    }
}

/// Return the first match position on a line (first for forward, last for backward).
pub(crate) fn first_match(
    doc: &mut Document,
    query: &Regex,
    line_index: usize,
    direction: SearchDirection,
) -> Option<MatchPosition> {
    let line = doc.line(line_index)?;
    let matches = line.find_matches(query);
    // let match_count = line.find_matches(query).len();
    if matches.is_empty() {
        return None;
    }
    let match_index = match direction {
        SearchDirection::Forward => 0,
        SearchDirection::Backward => matches.len() - 1,
    };
    let m = matches[match_index];
    Some(MatchPosition {
        line_index,
        plain_range: m.0..m.1,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_doc(lines: &[&str]) -> Document {
        Document::from_string(lines.join("\n"))
    }

    fn make_query(pattern: &str) -> Regex {
        Regex::new(&regex::escape(pattern)).unwrap()
    }

    fn pos(line_index: usize, plain_range: Range<usize>) -> Option<MatchPosition> {
        Some(MatchPosition {
            line_index,
            plain_range,
        })
    }

    #[test]
    fn forward_finds_first_match() {
        let mut doc = make_doc(&["aaa", "bbb", "ccc", "bbb"]);
        let query = make_query("bbb");
        assert_eq!(
            find_next_match(&mut doc, &query, 0, SearchDirection::Forward),
            pos(1, 0..3)
        );
    }

    #[test]
    fn forward_wraps_around() {
        let mut doc = make_doc(&["aaa", "bbb", "ccc"]);
        let query = make_query("aaa");
        // Starting from line 2, should wrap to line 0
        assert_eq!(
            find_next_match(&mut doc, &query, 2, SearchDirection::Forward),
            pos(0, 0..3)
        );
    }

    #[test]
    fn backward_finds_match() {
        let mut doc = make_doc(&["aaa", "bbb", "ccc", "bbb"]);
        let query = make_query("bbb");
        assert_eq!(
            find_next_match(&mut doc, &query, 3, SearchDirection::Backward),
            pos(3, 0..3)
        );
    }

    #[test]
    fn backward_wraps_around() {
        let mut doc = make_doc(&["aaa", "bbb", "ccc"]);
        let query = make_query("ccc");
        // Starting from line 0, should wrap to line 2
        assert_eq!(
            find_next_match(&mut doc, &query, 0, SearchDirection::Backward),
            pos(2, 0..3)
        );
    }

    #[test]
    fn no_match_returns_none() {
        let mut doc = make_doc(&["aaa", "bbb", "ccc"]);
        let query = make_query("zzz");
        assert_eq!(
            find_next_match(&mut doc, &query, 0, SearchDirection::Forward),
            None
        );
    }

    #[test]
    fn empty_document() {
        let mut doc = make_doc(&[]);
        let query = make_query("foo");
        assert_eq!(
            find_next_match(&mut doc, &query, 0, SearchDirection::Forward),
            None
        );
    }

    #[test]
    fn forward_skips_current_line_on_wrap() {
        let mut doc = make_doc(&["aaa", "bbb", "ccc"]);
        let query = make_query("bbb");
        // Start from line 2, forward: checks 2, then wraps to 0, 1 -> finds 1
        assert_eq!(
            find_next_match(&mut doc, &query, 2, SearchDirection::Forward),
            pos(1, 0..3)
        );
    }
}
