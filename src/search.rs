/// Search state and navigation for finding matches in a document.
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

/// Active search state, preserved across search submissions.
pub struct SearchState {
    pub query: Regex,
    pub direction: SearchDirection,
    /// Line index of the current match position.
    pub current_line: Option<usize>,
}

/// Find the next match from `from_line` in the given direction.
/// Wraps around the document if no match is found before the end/start.
pub fn find_next_match(
    doc: &mut Document,
    query: &Regex,
    from_line: usize,
    direction: SearchDirection,
) -> Option<usize> {
    let line_count = doc.line_count();
    if line_count == 0 {
        return None;
    }

    match direction {
        SearchDirection::Forward => {
            // Search from_line..end, then 0..from_line (wrap around)
            for i in from_line..line_count {
                if has_match(doc, query, i) {
                    return Some(i);
                }
            }
            for i in 0..from_line {
                if has_match(doc, query, i) {
                    return Some(i);
                }
            }
        }
        SearchDirection::Backward => {
            // Search from_line..0 (reverse), then end..from_line (reverse, wrap around)
            for i in (0..=from_line).rev() {
                if has_match(doc, query, i) {
                    return Some(i);
                }
            }
            for i in (from_line + 1..line_count).rev() {
                if has_match(doc, query, i) {
                    return Some(i);
                }
            }
        }
    }

    None
}

/// Check if a line has any match for the query.
fn has_match(doc: &mut Document, query: &Regex, line_index: usize) -> bool {
    doc.line(line_index)
        .map(|line| query.is_match(line.plain()))
        .unwrap_or(false)
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

    #[test]
    fn forward_finds_first_match() {
        let mut doc = make_doc(&["aaa", "bbb", "ccc", "bbb"]);
        let query = make_query("bbb");
        assert_eq!(
            find_next_match(&mut doc, &query, 0, SearchDirection::Forward),
            Some(1)
        );
    }

    #[test]
    fn forward_wraps_around() {
        let mut doc = make_doc(&["aaa", "bbb", "ccc"]);
        let query = make_query("aaa");
        // Starting from line 2, should wrap to line 0
        assert_eq!(
            find_next_match(&mut doc, &query, 2, SearchDirection::Forward),
            Some(0)
        );
    }

    #[test]
    fn backward_finds_match() {
        let mut doc = make_doc(&["aaa", "bbb", "ccc", "bbb"]);
        let query = make_query("bbb");
        assert_eq!(
            find_next_match(&mut doc, &query, 3, SearchDirection::Backward),
            Some(3)
        );
    }

    #[test]
    fn backward_wraps_around() {
        let mut doc = make_doc(&["aaa", "bbb", "ccc"]);
        let query = make_query("ccc");
        // Starting from line 0, should wrap to line 2
        assert_eq!(
            find_next_match(&mut doc, &query, 0, SearchDirection::Backward),
            Some(2)
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
            Some(1)
        );
    }
}
