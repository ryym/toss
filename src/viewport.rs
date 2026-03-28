use std::num::NonZeroUsize;

use crate::document::Document;

/// Identifies a single screen row: which document line, which wrap row within it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScreenRow {
    pub line_index: usize,
    pub wrap_index: usize,
}

/// Direction of scrolling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Down,
    Up,
}

/// Instructions for incremental rendering after a scroll operation.
/// Only constructed when scrolling actually occurs.
#[derive(Debug)]
pub struct ScrollPlan {
    /// How many rows to scroll the terminal (always non-zero).
    pub terminal_scroll: NonZeroUsize,
    pub direction: Direction,
}

/// Move one row forward in the document. Returns None at the end.
fn next_row(doc: &mut Document, width: usize, row: ScreenRow) -> Option<ScreenRow> {
    let line = doc.line(row.line_index)?;
    if row.wrap_index + 1 < line.row_count(width) {
        Some(ScreenRow {
            line_index: row.line_index,
            wrap_index: row.wrap_index + 1,
        })
    } else {
        doc.line(row.line_index + 1)?;
        Some(ScreenRow {
            line_index: row.line_index + 1,
            wrap_index: 0,
        })
    }
}

/// Move one row backward in the document. Returns None at the beginning.
fn prev_row(doc: &mut Document, width: usize, row: ScreenRow) -> Option<ScreenRow> {
    if row.wrap_index > 0 {
        Some(ScreenRow {
            line_index: row.line_index,
            wrap_index: row.wrap_index - 1,
        })
    } else if row.line_index > 0 {
        let prev_line = doc.line(row.line_index - 1)?;
        Some(ScreenRow {
            line_index: row.line_index - 1,
            wrap_index: prev_line.row_count(width) - 1,
        })
    } else {
        None
    }
}

/// Viewport of the content area (excludes header, status line, etc.).
/// Tracks which document rows are currently visible and provides scroll
/// operations that return minimal diffs for rendering.
pub struct Viewport {
    /// What each screen row currently shows.
    rows: Vec<ScreenRow>,
    /// Width of the content area.
    width: usize,
    /// Height of the content area.
    height: usize,
    /// Number of fixed header lines. The viewport cannot scroll above this line.
    fixed_line_len: usize,
}

impl Viewport {
    /// Build initial viewport from the top of the document.
    /// `width` and `height` are the content area dimensions
    /// (excluding status line, header, etc.).
    /// `fixed_line_len` is the number of fixed header lines. The viewport
    /// starts after these lines.
    pub fn new(doc: &mut Document, width: usize, height: usize, fixed_line_len: usize) -> Self {
        let rows = Self::build_rows_forward(doc, width, fixed_line_len, 0, height);
        Self {
            rows,
            width,
            height,
            fixed_line_len,
        }
    }

    /// Width of the content area.
    pub fn width(&self) -> usize {
        self.width
    }

    /// Height of the content area.
    pub fn height(&self) -> usize {
        self.height
    }

    /// Current screen rows.
    pub fn rows(&self) -> &[ScreenRow] {
        &self.rows
    }

    /// Line index of the first visible row, or 0 if no rows exist.
    pub fn top_line_index(&self) -> usize {
        self.rows.first().map(|r| r.line_index).unwrap_or(0)
    }

    /// Wrap index of the first visible row, or 0 if no rows exist.
    pub fn top_wrap_index(&self) -> usize {
        self.rows.first().map(|r| r.wrap_index).unwrap_or(0)
    }

    /// Scroll down by n screen rows. Returns `None` if no scrolling occurred.
    pub fn scroll_down(&mut self, n: usize, doc: &mut Document) -> Option<ScrollPlan> {
        if n == 0 || self.rows.is_empty() {
            return None;
        }

        let height = self.rows.len();

        // Find new rows to add at the bottom
        let last = self.rows[height - 1];
        let new_rows = Self::advance_forward(doc, self.width, last, n);
        let actual = NonZeroUsize::new(new_rows.len())?;

        // Update rows: remove `actual` from top, add new at bottom
        self.rows.drain(..actual.get());
        self.rows.extend_from_slice(&new_rows);

        Some(ScrollPlan {
            terminal_scroll: actual,
            direction: Direction::Down,
        })
    }

    /// Scroll up by n screen rows. Returns `None` if no scrolling occurred.
    pub fn scroll_up(&mut self, n: usize, doc: &mut Document) -> Option<ScrollPlan> {
        if n == 0 || self.rows.is_empty() {
            return None;
        }

        let first = self.rows[0];
        let new_rows = Self::advance_backward(doc, self.width, first, n, self.fixed_line_len);
        let actual = NonZeroUsize::new(new_rows.len())?;

        // Update rows: remove `actual` from bottom, prepend new at top
        let height = self.rows.len();
        self.rows.truncate(height - actual.get());
        // Prepend: new_rows is in top-to-bottom order
        let mut new_vec = new_rows.clone();
        new_vec.append(&mut self.rows);
        self.rows = new_vec;

        Some(ScrollPlan {
            terminal_scroll: actual,
            direction: Direction::Up,
        })
    }

    /// Jump to a specific line, rebuilding the screen from there.
    /// Returns None if the position hasn't changed.
    pub fn jump_to(&mut self, doc: &mut Document, line_index: usize) -> bool {
        let line_index = line_index.max(self.fixed_line_len);
        let height = self.rows.len();
        let mut new_rows = Self::build_rows_forward(doc, self.width, line_index, 0, height);
        if new_rows.len() < height {
            // Near end of document: back-fill from the end to keep the screen full.
            new_rows =
                Self::build_rows_backward_from_end(doc, self.width, height, self.fixed_line_len);
        }
        if new_rows == self.rows {
            return false;
        }
        self.rows = new_rows;
        true
    }

    /// Jump to the end of the document so that the last line is at the bottom.
    pub fn jump_to_end(&mut self, doc: &mut Document) -> bool {
        let height = self.rows.len();
        let new_rows =
            Self::build_rows_backward_from_end(doc, self.width, height, self.fixed_line_len);
        if new_rows == self.rows {
            return false;
        }
        self.rows = new_rows;
        true
    }

    /// Update dimensions and rebuild from current top position.
    /// `width` and `height` are the content area dimensions.
    pub fn resize(&mut self, doc: &mut Document, width: usize, height: usize) {
        let top = self.rows.first().copied().unwrap_or(ScreenRow {
            line_index: self.fixed_line_len,
            wrap_index: 0,
        });
        let top_line = top.line_index.max(self.fixed_line_len);
        self.width = width;
        self.height = height;
        self.rows = Self::build_rows_forward(doc, width, top_line, top.wrap_index, height);
    }

    /// Build rows starting from (line_index, wrap_index), going forward.
    fn build_rows_forward(
        doc: &mut Document,
        width: usize,
        start_line: usize,
        start_wrap: usize,
        count: usize,
    ) -> Vec<ScreenRow> {
        if count == 0 || doc.line(start_line).is_none() {
            return vec![];
        }
        let mut current = ScreenRow {
            line_index: start_line,
            wrap_index: start_wrap,
        };
        let mut rows = vec![current];
        while rows.len() < count {
            let Some(next) = next_row(doc, width, current) else {
                break;
            };
            rows.push(next);
            current = next;
        }
        rows
    }

    /// Build rows from the end of the document, filling up to `count` rows.
    fn build_rows_backward_from_end(
        doc: &mut Document,
        width: usize,
        count: usize,
        fixed_line_len: usize,
    ) -> Vec<ScreenRow> {
        let line_count = doc.line_count();
        if line_count == 0 || line_count <= fixed_line_len {
            return vec![];
        }
        let last_line = doc.line(line_count - 1).unwrap();
        let mut current = ScreenRow {
            line_index: line_count - 1,
            wrap_index: last_line.row_count(width) - 1,
        };
        let mut rows = vec![current];
        while rows.len() < count {
            let Some(prev) = prev_row(doc, width, current) else {
                break;
            };
            if prev.line_index < fixed_line_len {
                break;
            }
            rows.push(prev);
            current = prev;
        }
        rows.reverse();
        rows
    }

    /// Advance forward from `after` by `n` screen rows.
    fn advance_forward(
        doc: &mut Document,
        width: usize,
        after: ScreenRow,
        n: usize,
    ) -> Vec<ScreenRow> {
        let mut rows = Vec::with_capacity(n);
        let mut current = after;
        for _ in 0..n {
            let Some(next) = next_row(doc, width, current) else {
                break;
            };
            rows.push(next);
            current = next;
        }
        rows
    }

    /// Advance backward from `before` by `n` screen rows.
    /// Returns rows in top-to-bottom order.
    fn advance_backward(
        doc: &mut Document,
        width: usize,
        before: ScreenRow,
        n: usize,
        fixed_line_len: usize,
    ) -> Vec<ScreenRow> {
        let mut rows = Vec::with_capacity(n);
        let mut current = before;
        for _ in 0..n {
            let Some(prev) = prev_row(doc, width, current) else {
                break;
            };
            if prev.line_index < fixed_line_len {
                break;
            }
            rows.push(prev);
            current = prev;
        }
        rows.reverse();
        rows
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn make_doc(lines: &[&str]) -> Document {
        Document::from_string(lines.join("\n"))
    }

    #[test]
    fn initial_state_simple() {
        let mut doc = make_doc(&["aaa", "bbb", "ccc", "ddd", "eee"]);
        let state = Viewport::new(&mut doc, 80, 3, 0);
        assert_eq!(
            state.rows(),
            &[
                ScreenRow {
                    line_index: 0,
                    wrap_index: 0
                },
                ScreenRow {
                    line_index: 1,
                    wrap_index: 0
                },
                ScreenRow {
                    line_index: 2,
                    wrap_index: 0
                },
            ]
        );
    }

    #[test]
    fn initial_state_with_wrapping() {
        // "abcdefgh" wraps to 2 rows at width 5
        let mut doc = make_doc(&["abcdefgh", "xy"]);
        let state = Viewport::new(&mut doc, 5, 4, 0);
        assert_eq!(
            state.rows(),
            &[
                ScreenRow {
                    line_index: 0,
                    wrap_index: 0
                },
                ScreenRow {
                    line_index: 0,
                    wrap_index: 1
                },
                ScreenRow {
                    line_index: 1,
                    wrap_index: 0
                },
            ]
        );
    }

    #[test]
    fn scroll_down_one() {
        let mut doc = make_doc(&["aaa", "bbb", "ccc", "ddd"]);
        let mut state = Viewport::new(&mut doc, 80, 3, 0);
        let plan = state.scroll_down(1, &mut doc).unwrap();

        assert_eq!(plan.terminal_scroll.get(), 1);
        assert_eq!(plan.direction, Direction::Down);
        assert_eq!(
            state.rows(),
            &[
                ScreenRow {
                    line_index: 1,
                    wrap_index: 0
                },
                ScreenRow {
                    line_index: 2,
                    wrap_index: 0
                },
                ScreenRow {
                    line_index: 3,
                    wrap_index: 0
                },
            ]
        );
    }

    #[test]
    fn scroll_down_at_end() {
        let mut doc = make_doc(&["aaa", "bbb", "ccc"]);
        let mut state = Viewport::new(&mut doc, 80, 3, 0);
        assert!(state.scroll_down(1, &mut doc).is_none());
        // State unchanged
        assert_eq!(state.rows()[0].line_index, 0);
    }

    #[test]
    fn scroll_down_with_wrap() {
        // Line "abcdefgh" wraps to 2 rows at width 5
        let mut doc = make_doc(&["short", "abcdefgh", "end"]);
        let mut state = Viewport::new(&mut doc, 5, 3, 0);
        // Initial: [short/0, abcde/0, fgh/1]
        assert_eq!(
            state.rows()[0],
            ScreenRow {
                line_index: 0,
                wrap_index: 0
            }
        );

        let plan = state.scroll_down(1, &mut doc).unwrap();
        assert_eq!(plan.terminal_scroll.get(), 1);
        // After: [abcde/0, fgh/1, end/0]
        assert_eq!(
            state.rows(),
            &[
                ScreenRow {
                    line_index: 1,
                    wrap_index: 0
                },
                ScreenRow {
                    line_index: 1,
                    wrap_index: 1
                },
                ScreenRow {
                    line_index: 2,
                    wrap_index: 0
                },
            ]
        );
    }

    #[test]
    fn scroll_up_one() {
        let mut doc = make_doc(&["aaa", "bbb", "ccc", "ddd"]);
        let mut state = Viewport::new(&mut doc, 80, 3, 0);
        state.scroll_down(1, &mut doc);
        // Now: [bbb, ccc, ddd]

        let plan = state.scroll_up(1, &mut doc).unwrap();
        assert_eq!(plan.terminal_scroll.get(), 1);
        assert_eq!(plan.direction, Direction::Up);
        assert_eq!(
            state.rows(),
            &[
                ScreenRow {
                    line_index: 0,
                    wrap_index: 0
                },
                ScreenRow {
                    line_index: 1,
                    wrap_index: 0
                },
                ScreenRow {
                    line_index: 2,
                    wrap_index: 0
                },
            ]
        );
    }

    #[test]
    fn scroll_up_at_top() {
        let mut doc = make_doc(&["aaa", "bbb"]);
        let mut state = Viewport::new(&mut doc, 80, 2, 0);
        assert!(state.scroll_up(1, &mut doc).is_none());
    }

    #[test]
    fn scroll_up_with_wrap() {
        let mut doc = make_doc(&["abcdefgh", "short"]);
        let mut state = Viewport::new(&mut doc, 5, 2, 0);
        // Initial: [abcde/0, fgh/1]
        state.scroll_down(1, &mut doc);
        // Now: [fgh/1, short/0]

        let plan = state.scroll_up(1, &mut doc).unwrap();
        assert_eq!(plan.terminal_scroll.get(), 1);
        // Back to: [abcde/0, fgh/1]
        assert_eq!(
            state.rows(),
            &[
                ScreenRow {
                    line_index: 0,
                    wrap_index: 0
                },
                ScreenRow {
                    line_index: 0,
                    wrap_index: 1
                },
            ]
        );
    }

    #[test]
    fn scroll_down_multiple() {
        let mut doc = make_doc(&["a", "b", "c", "d", "e", "f"]);
        let mut state = Viewport::new(&mut doc, 80, 3, 0);
        let plan = state.scroll_down(3, &mut doc).unwrap();

        assert_eq!(plan.terminal_scroll.get(), 3);
        assert_eq!(
            state.rows(),
            &[
                ScreenRow {
                    line_index: 3,
                    wrap_index: 0
                },
                ScreenRow {
                    line_index: 4,
                    wrap_index: 0
                },
                ScreenRow {
                    line_index: 5,
                    wrap_index: 0
                },
            ]
        );
    }

    #[test]
    fn scroll_down_clamps_at_end() {
        let mut doc = make_doc(&["a", "b", "c", "d"]);
        let mut state = Viewport::new(&mut doc, 80, 3, 0);
        // Try to scroll down by 10, but only 1 row available
        let plan = state.scroll_down(10, &mut doc).unwrap();

        assert_eq!(plan.terminal_scroll.get(), 1);
        assert_eq!(
            state.rows(),
            &[
                ScreenRow {
                    line_index: 1,
                    wrap_index: 0
                },
                ScreenRow {
                    line_index: 2,
                    wrap_index: 0
                },
                ScreenRow {
                    line_index: 3,
                    wrap_index: 0
                },
            ]
        );
    }

    #[test]
    fn jump_to_line() {
        let mut doc = make_doc(&["a", "b", "c", "d", "e"]);
        let mut state = Viewport::new(&mut doc, 80, 3, 0);
        let changed = state.jump_to(&mut doc, 2);

        assert!(changed);
        assert_eq!(
            state.rows(),
            &[
                ScreenRow {
                    line_index: 2,
                    wrap_index: 0
                },
                ScreenRow {
                    line_index: 3,
                    wrap_index: 0
                },
                ScreenRow {
                    line_index: 4,
                    wrap_index: 0
                },
            ]
        );
    }

    #[test]
    fn jump_to_same_position() {
        let mut doc = make_doc(&["a", "b", "c"]);
        let mut state = Viewport::new(&mut doc, 80, 3, 0);
        let changed = state.jump_to(&mut doc, 0);
        assert!(!changed);
    }

    #[test]
    fn fewer_lines_than_height() {
        let mut doc = make_doc(&["a", "b"]);
        let state = Viewport::new(&mut doc, 80, 5, 0);
        assert_eq!(state.rows().len(), 2);
    }

    #[test]
    fn jump_to_end() {
        let mut doc = make_doc(&["a", "b", "c", "d", "e"]);
        let mut state = Viewport::new(&mut doc, 80, 3, 0);
        let changed = state.jump_to_end(&mut doc);

        assert!(changed);
        assert_eq!(
            state.rows(),
            &[
                ScreenRow {
                    line_index: 2,
                    wrap_index: 0
                },
                ScreenRow {
                    line_index: 3,
                    wrap_index: 0
                },
                ScreenRow {
                    line_index: 4,
                    wrap_index: 0
                },
            ]
        );
    }

    #[test]
    fn jump_to_end_with_wrap() {
        // Last line "abcdefgh" wraps to 2 rows at width 5
        let mut doc = make_doc(&["a", "b", "abcdefgh"]);
        let mut state = Viewport::new(&mut doc, 5, 3, 0);
        let changed = state.jump_to_end(&mut doc);

        assert!(changed);
        assert_eq!(
            state.rows(),
            &[
                ScreenRow {
                    line_index: 1,
                    wrap_index: 0
                },
                ScreenRow {
                    line_index: 2,
                    wrap_index: 0
                },
                ScreenRow {
                    line_index: 2,
                    wrap_index: 1
                },
            ]
        );
    }

    #[test]
    fn jump_to_end_fewer_lines_than_height() {
        let mut doc = make_doc(&["a", "b"]);
        let mut state = Viewport::new(&mut doc, 80, 5, 0);
        let changed = state.jump_to_end(&mut doc);
        // Already showing everything, no change
        assert!(!changed);
    }
}
