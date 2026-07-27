//! Search state and navigation for finding matches in a document.

use regex::Regex;

use crate::{
    document::Document,
    line::{MatchPosition, Row, SearchLineFrom},
};

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

/// Active search state, preserved across search submissions.
#[derive(Debug)]
pub struct SearchState {
    pub query: Regex,
    pub direction: SearchDirection,
    /// Current match position (line and which match within the line).
    pub current: Option<MatchPosition>,
}

/// Specifies the starting position when searching across a [`Document`].
/// The direction is specified separately via [`SearchDirection`].
pub enum SearchFrom {
    /// Search from the line at the given index onward.
    Line(usize),
    /// Search from the given [`Row`] onward. Only [`SearchDirection::Forward`] is supported.
    Row(Row),
    /// Search after the given match. The match itself is excluded.
    NextOf(MatchPosition),
}

/// Find the next match from `from` in the given direction.
/// Wraps around the document if no match is found before the end/start.
pub fn search_document(
    doc: &mut Document,
    query: &Regex,
    from: SearchFrom,
    direction: SearchDirection,
) -> Option<MatchPosition> {
    match from {
        SearchFrom::Line(line_index) => match direction {
            SearchDirection::Forward => {
                search_forward(doc, query, line_index, SearchLineFrom::Start)
            }
            SearchDirection::Backward => {
                search_backward(doc, query, line_index, SearchLineFrom::Start)
            }
        },
        SearchFrom::Row(row) => match direction {
            SearchDirection::Forward => {
                search_forward(doc, query, row.line_index(), SearchLineFrom::Row(row))
            }
            SearchDirection::Backward => {
                panic!("Searching document backwards from row is not supported")
            }
        },
        SearchFrom::NextOf(m) => match direction {
            SearchDirection::Forward => {
                search_forward(doc, query, m.line_index(), SearchLineFrom::NextOf(m))
            }
            SearchDirection::Backward => {
                search_backward(doc, query, m.line_index(), SearchLineFrom::PrevOf(m))
            }
        },
    }
}

fn search_forward(
    doc: &mut Document,
    query: &Regex,
    from_line: usize,
    first_line_from: SearchLineFrom,
) -> Option<MatchPosition> {
    // Search a next match in the same line.
    let line = doc.line(from_line)?;
    if let Some(m) = line.find_first_match_from(query, first_line_from) {
        return Some(m);
    }
    // Search from_line+1 to bottom, then top to from_line (wrap around).
    let next_line = from_line + 1;
    let line_count = doc.line_count();
    for i in (next_line..line_count).chain(0..next_line) {
        if let Some(m) = doc
            .line(i)?
            .find_first_match_from(query, SearchLineFrom::Start)
        {
            return Some(m);
        }
    }
    None
}

fn search_backward(
    doc: &mut Document,
    query: &Regex,
    from_line: usize,
    first_line_from: SearchLineFrom,
) -> Option<MatchPosition> {
    // Search a next match in the same line.
    let line = doc.line(from_line)?;
    if let Some(m) = line.find_last_match_from(query, first_line_from) {
        return Some(m);
    }
    // Search from_line-1 to top, then bottom to from_line (wrap around).
    let line_count = doc.line_count();
    for i in (0..from_line).rev().chain((from_line..line_count).rev()) {
        if let Some(m) = doc
            .line(i)?
            .find_last_match_from(query, SearchLineFrom::Start)
        {
            return Some(m);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use std::ops::Range;

    use super::*;

    fn make_doc(lines: &[&str]) -> Document {
        Document::from_string(lines.join("\n"))
    }

    fn make_query(pattern: &str) -> Regex {
        Regex::new(&regex::escape(pattern)).unwrap()
    }

    fn pos(line_index: usize, plain_range: Range<usize>) -> Option<MatchPosition> {
        Some(MatchPosition::new(line_index, plain_range))
    }

    // --- search_document tests ---

    #[test]
    fn document_line_forward_finds_match() {
        let mut doc = make_doc(&["aaa", "bbb cc bbb", "ddd"]);
        let query = make_query("bbb");
        // Starts from line 1, picks the first match on it.
        assert_eq!(
            search_document(
                &mut doc,
                &query,
                SearchFrom::Line(1),
                SearchDirection::Forward
            ),
            pos(1, 0..3)
        );
    }

    #[test]
    fn document_line_forward_wraps_around() {
        let mut doc = make_doc(&["aaa", "bbb", "ccc"]);
        let query = make_query("aaa");
        // From line 2 there is no match; wraps to line 0.
        assert_eq!(
            search_document(
                &mut doc,
                &query,
                SearchFrom::Line(2),
                SearchDirection::Forward
            ),
            pos(0, 0..3)
        );
    }

    #[test]
    fn document_line_backward_finds_last_match_in_line() {
        let mut doc = make_doc(&["aaa", "bbb cc bbb", "ddd"]);
        let query = make_query("bbb");
        // Starts from line 1; picks the last match on that line.
        assert_eq!(
            search_document(
                &mut doc,
                &query,
                SearchFrom::Line(1),
                SearchDirection::Backward
            ),
            pos(1, 7..10)
        );
    }

    #[test]
    fn document_line_backward_wraps_around() {
        let mut doc = make_doc(&["aaa", "bbb", "ccc"]);
        let query = make_query("ccc");
        // From line 0 there is no match; wraps to line 2.
        assert_eq!(
            search_document(
                &mut doc,
                &query,
                SearchFrom::Line(0),
                SearchDirection::Backward
            ),
            pos(2, 0..3)
        );
    }

    #[test]
    fn document_next_of_forward_same_line() {
        let mut doc = make_doc(&["xx", "ab cd ab ef ab", "yy"]);
        let query = make_query("ab");
        let border = MatchPosition::new(1, 0..2);
        assert_eq!(
            search_document(
                &mut doc,
                &query,
                SearchFrom::NextOf(border.clone()),
                SearchDirection::Forward,
            ),
            pos(1, 6..8)
        );
    }

    #[test]
    fn document_next_of_forward_advances_to_next_line() {
        let mut doc = make_doc(&["xx", "ab cd", "yy ab"]);
        let query = make_query("ab");
        let border = MatchPosition::new(1, 0..2);
        // No further match on line 1; advances to line 2.
        assert_eq!(
            search_document(
                &mut doc,
                &query,
                SearchFrom::NextOf(border.clone()),
                SearchDirection::Forward,
            ),
            pos(2, 3..5)
        );
    }

    #[test]
    fn document_next_of_forward_wraps_to_earlier_match_on_same_line() {
        // Only line 1 has matches. After exhausting forward search wraps around
        // and picks up the match before border on the same line.
        let mut doc = make_doc(&["xx", "ab cd ab", "yy"]);
        let query = make_query("ab");
        let border = MatchPosition::new(1, 6..8);
        assert_eq!(
            search_document(
                &mut doc,
                &query,
                SearchFrom::NextOf(border.clone()),
                SearchDirection::Forward,
            ),
            pos(1, 0..2)
        );
    }

    #[test]
    fn document_next_of_backward_same_line() {
        let mut doc = make_doc(&["xx", "ab cd ab ef ab", "yy"]);
        let query = make_query("ab");
        let border = MatchPosition::new(1, 12..14);
        assert_eq!(
            search_document(
                &mut doc,
                &query,
                SearchFrom::NextOf(border.clone()),
                SearchDirection::Backward,
            ),
            pos(1, 6..8)
        );
    }

    #[test]
    fn document_next_of_backward_wraps_to_later_match_on_same_line() {
        // Only line 1 has matches. After exhausting backward search wraps around
        // and picks up the match after border on the same line.
        let mut doc = make_doc(&["xx", "ab cd ab", "yy"]);
        let query = make_query("ab");
        let border = MatchPosition::new(1, 0..2);
        assert_eq!(
            search_document(
                &mut doc,
                &query,
                SearchFrom::NextOf(border.clone()),
                SearchDirection::Backward,
            ),
            pos(1, 6..8)
        );
    }

    #[test]
    fn document_row_forward_skips_matches_before_row() {
        let mut doc = make_doc(&["abcdefghij"]);
        let query = make_query("cd");
        // Wrap line 0 at width 5: row 0 = "abcde", row 1 = "fghij".
        let rows = doc.line(0).unwrap().wrap(5);
        // Searching from row 1 skips the "cd" match in row 0; no match remains
        // on the line, so wraps around the document and returns "cd" again
        // via the wrap-around revisit of line 0.
        assert_eq!(
            search_document(
                &mut doc,
                &query,
                SearchFrom::Row(rows[1].clone()),
                SearchDirection::Forward,
            ),
            pos(0, 2..4)
        );
    }

    #[test]
    fn document_no_match_returns_none() {
        let mut doc = make_doc(&["aaa", "bbb"]);
        let query = make_query("zzz");
        assert_eq!(
            search_document(
                &mut doc,
                &query,
                SearchFrom::Line(0),
                SearchDirection::Forward
            ),
            None
        );
    }

    #[test]
    fn document_empty_returns_none() {
        let mut doc = make_doc(&[]);
        let query = make_query("foo");
        assert_eq!(
            search_document(
                &mut doc,
                &query,
                SearchFrom::Line(0),
                SearchDirection::Forward
            ),
            None
        );
    }

    // --- regex syntax tests ---

    #[test]
    fn character_class_matches() {
        let mut doc = make_doc(&["aaa", "bcd", "zzz"]);
        let query = Regex::new("[bc]+").unwrap();
        assert_eq!(
            search_document(
                &mut doc,
                &query,
                SearchFrom::Line(0),
                SearchDirection::Forward
            ),
            pos(1, 0..2)
        );
    }

    #[test]
    fn quantifier_matches() {
        let mut doc = make_doc(&["a", "aaa", "aa"]);
        let query = Regex::new("a{3}").unwrap();
        assert_eq!(
            search_document(
                &mut doc,
                &query,
                SearchFrom::Line(0),
                SearchDirection::Forward
            ),
            pos(1, 0..3)
        );
    }

    #[test]
    fn anchor_matches_line_start() {
        let mut doc = make_doc(&["bab", "abc"]);
        let query = Regex::new("^a").unwrap();
        assert_eq!(
            search_document(
                &mut doc,
                &query,
                SearchFrom::Line(0),
                SearchDirection::Forward
            ),
            pos(1, 0..1)
        );
    }

    // Zero-width matches (e.g. `a*` matching an empty string) must not cause
    // an infinite loop; the regex crate advances by one byte on empty matches.
    #[test]
    fn zero_width_match_does_not_hang() {
        let mut doc = make_doc(&["bbb", "aaa"]);
        let query = Regex::new("a*").unwrap();
        assert_eq!(
            search_document(
                &mut doc,
                &query,
                SearchFrom::Line(1),
                SearchDirection::Forward
            ),
            pos(1, 0..3)
        );
    }
}
