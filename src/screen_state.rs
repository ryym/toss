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
#[derive(Debug)]
pub struct ScrollPlan {
    /// How many rows to scroll the terminal.
    pub terminal_scroll: usize,
    pub direction: Direction,
    /// New rows to draw (at the top or bottom depending on direction).
    pub new_rows: Vec<ScreenRow>,
}

/// Tracks what is currently displayed on each screen row.
/// Provides scroll operations that return minimal diffs for rendering.
pub struct ScreenState {
    /// What each screen row currently shows.
    rows: Vec<ScreenRow>,
    /// Full terminal width.
    width: usize,
    /// Full terminal height (content area + status line).
    height: usize,
}

impl ScreenState {
    /// Build initial screen state from the top of the document.
    /// `width` and `height` are the full terminal dimensions.
    /// The bottom row is reserved for the status line.
    pub fn new(doc: &mut Document, width: usize, height: usize) -> Self {
        let content_height = height.saturating_sub(1);
        let rows = Self::build_rows_forward(doc, width, 0, 0, content_height);
        Self {
            rows,
            width,
            height,
        }
    }

    /// Full terminal width.
    pub fn width(&self) -> usize {
        self.width
    }

    /// Number of rows available for document content (excludes the status line).
    pub fn content_height(&self) -> usize {
        self.height.saturating_sub(1)
    }

    /// Current screen rows.
    pub fn rows(&self) -> &[ScreenRow] {
        &self.rows
    }

    /// Scroll down by n screen rows. Returns a plan for incremental rendering.
    pub fn scroll_down(&mut self, n: usize, doc: &mut Document) -> ScrollPlan {
        if n == 0 || self.rows.is_empty() {
            return ScrollPlan {
                terminal_scroll: 0,
                direction: Direction::Down,
                new_rows: vec![],
            };
        }

        let height = self.rows.len();

        // Find new rows to add at the bottom
        let last = self.rows[height - 1];
        let new_rows = Self::advance_forward(doc, self.width, last, n);
        let actual = new_rows.len();

        if actual == 0 {
            return ScrollPlan {
                terminal_scroll: 0,
                direction: Direction::Down,
                new_rows: vec![],
            };
        }

        // Update rows: remove `actual` from top, add new at bottom
        self.rows.drain(..actual);
        self.rows.extend_from_slice(&new_rows);

        ScrollPlan {
            terminal_scroll: actual,
            direction: Direction::Down,
            new_rows,
        }
    }

    /// Scroll up by n screen rows. Returns a plan for incremental rendering.
    pub fn scroll_up(&mut self, n: usize, doc: &mut Document) -> ScrollPlan {
        if n == 0 || self.rows.is_empty() {
            return ScrollPlan {
                terminal_scroll: 0,
                direction: Direction::Up,
                new_rows: vec![],
            };
        }

        let first = self.rows[0];
        let new_rows = Self::advance_backward(doc, self.width, first, n);
        let actual = new_rows.len();

        if actual == 0 {
            return ScrollPlan {
                terminal_scroll: 0,
                direction: Direction::Up,
                new_rows: vec![],
            };
        }

        // Update rows: remove `actual` from bottom, prepend new at top
        let height = self.rows.len();
        self.rows.truncate(height - actual);
        // Prepend: new_rows is in top-to-bottom order
        let mut new_vec = new_rows.clone();
        new_vec.append(&mut self.rows);
        self.rows = new_vec;

        ScrollPlan {
            terminal_scroll: actual,
            direction: Direction::Up,
            new_rows,
        }
    }

    /// Jump to a specific line, rebuilding the screen from there.
    /// Returns None if the position hasn't changed.
    pub fn jump_to(&mut self, doc: &mut Document, line_index: usize) -> bool {
        let height = self.rows.len();
        let new_rows = Self::build_rows_forward(doc, self.width, line_index, 0, height);
        if new_rows == self.rows {
            return false;
        }
        self.rows = new_rows;
        true
    }

    /// Jump to the end of the document so that the last line is at the bottom.
    pub fn jump_to_end(&mut self, doc: &mut Document) -> bool {
        let height = self.rows.len();
        let new_rows = Self::build_rows_backward_from_end(doc, self.width, height);
        if new_rows == self.rows {
            return false;
        }
        self.rows = new_rows;
        true
    }

    /// Update dimensions and rebuild screen from current top position.
    /// `width` and `height` are the full terminal dimensions.
    pub fn resize(&mut self, doc: &mut Document, width: usize, height: usize) {
        let top = self.rows.first().copied().unwrap_or(ScreenRow {
            line_index: 0,
            wrap_index: 0,
        });
        self.width = width;
        self.height = height;
        let content_height = self.content_height();
        self.rows =
            Self::build_rows_forward(doc, width, top.line_index, top.wrap_index, content_height);
    }

    /// Build rows starting from (line_index, wrap_index), going forward.
    fn build_rows_forward(
        doc: &mut Document,
        width: usize,
        start_line: usize,
        start_wrap: usize,
        count: usize,
    ) -> Vec<ScreenRow> {
        let mut rows = Vec::with_capacity(count);
        let mut line_idx = start_line;
        let mut wrap_idx = start_wrap;

        while rows.len() < count {
            let Some(line) = doc.line(line_idx) else {
                break;
            };
            let wrap_count = line.row_count(width);
            while wrap_idx < wrap_count && rows.len() < count {
                rows.push(ScreenRow {
                    line_index: line_idx,
                    wrap_index: wrap_idx,
                });
                wrap_idx += 1;
            }
            line_idx += 1;
            wrap_idx = 0;
        }

        rows
    }

    /// Build rows from the end of the document, filling up to `count` rows.
    fn build_rows_backward_from_end(
        doc: &mut Document,
        width: usize,
        count: usize,
    ) -> Vec<ScreenRow> {
        let line_count = doc.line_count();
        if line_count == 0 {
            return vec![];
        }

        let mut rows = Vec::with_capacity(count);
        let mut line_idx = line_count - 1;

        loop {
            let line = doc.line(line_idx).unwrap();
            let wrap_count = line.row_count(width);
            for wrap_idx in (0..wrap_count).rev() {
                rows.push(ScreenRow {
                    line_index: line_idx,
                    wrap_index: wrap_idx,
                });
                if rows.len() >= count {
                    rows.reverse();
                    return rows;
                }
            }
            if line_idx == 0 {
                break;
            }
            line_idx -= 1;
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
        let mut line_idx = after.line_index;
        let mut wrap_idx = after.wrap_index;

        // Advance one step from `after`
        let Some(first_line) = doc.line(line_idx) else {
            return rows;
        };
        let wrap_count = first_line.row_count(width);
        if wrap_idx + 1 < wrap_count {
            wrap_idx += 1;
        } else {
            line_idx += 1;
            wrap_idx = 0;
        }

        while rows.len() < n {
            let Some(line) = doc.line(line_idx) else {
                break;
            };
            let wc = line.row_count(width);
            while wrap_idx < wc && rows.len() < n {
                rows.push(ScreenRow {
                    line_index: line_idx,
                    wrap_index: wrap_idx,
                });
                wrap_idx += 1;
            }
            line_idx += 1;
            wrap_idx = 0;
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
    ) -> Vec<ScreenRow> {
        let mut rows = Vec::with_capacity(n);
        let mut line_idx = before.line_index;
        let mut wrap_idx = before.wrap_index;

        for _ in 0..n {
            if wrap_idx > 0 {
                wrap_idx -= 1;
            } else if line_idx > 0 {
                line_idx -= 1;
                let line = doc.line(line_idx).unwrap();
                wrap_idx = line.row_count(width) - 1;
            } else {
                // Reached the top of the document
                break;
            }
            rows.push(ScreenRow {
                line_index: line_idx,
                wrap_index: wrap_idx,
            });
        }

        // Reverse: we collected bottom-to-top, but want top-to-bottom
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
        let state = ScreenState::new(&mut doc, 80, 4);
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
        let state = ScreenState::new(&mut doc, 5, 5);
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
        let mut state = ScreenState::new(&mut doc, 80, 4);
        let plan = state.scroll_down(1, &mut doc);

        assert_eq!(plan.terminal_scroll, 1);
        assert_eq!(plan.direction, Direction::Down);
        assert_eq!(
            plan.new_rows,
            vec![ScreenRow {
                line_index: 3,
                wrap_index: 0
            }]
        );
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
        let mut state = ScreenState::new(&mut doc, 80, 4);
        let plan = state.scroll_down(1, &mut doc);

        assert_eq!(plan.terminal_scroll, 0);
        assert_eq!(plan.new_rows, vec![]);
        // State unchanged
        assert_eq!(state.rows()[0].line_index, 0);
    }

    #[test]
    fn scroll_down_with_wrap() {
        // Line "abcdefgh" wraps to 2 rows at width 5
        let mut doc = make_doc(&["short", "abcdefgh", "end"]);
        let mut state = ScreenState::new(&mut doc, 5, 4);
        // Initial: [short/0, abcde/0, fgh/1]
        assert_eq!(
            state.rows()[0],
            ScreenRow {
                line_index: 0,
                wrap_index: 0
            }
        );

        let plan = state.scroll_down(1, &mut doc);
        assert_eq!(plan.terminal_scroll, 1);
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
        let mut state = ScreenState::new(&mut doc, 80, 4);
        state.scroll_down(1, &mut doc);
        // Now: [bbb, ccc, ddd]

        let plan = state.scroll_up(1, &mut doc);
        assert_eq!(plan.terminal_scroll, 1);
        assert_eq!(plan.direction, Direction::Up);
        assert_eq!(
            plan.new_rows,
            vec![ScreenRow {
                line_index: 0,
                wrap_index: 0
            }]
        );
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
        let mut state = ScreenState::new(&mut doc, 80, 3);
        let plan = state.scroll_up(1, &mut doc);

        assert_eq!(plan.terminal_scroll, 0);
        assert_eq!(plan.new_rows, vec![]);
    }

    #[test]
    fn scroll_up_with_wrap() {
        let mut doc = make_doc(&["abcdefgh", "short"]);
        let mut state = ScreenState::new(&mut doc, 5, 3);
        // Initial: [abcde/0, fgh/1]
        state.scroll_down(1, &mut doc);
        // Now: [fgh/1, short/0]

        let plan = state.scroll_up(1, &mut doc);
        assert_eq!(plan.terminal_scroll, 1);
        assert_eq!(
            plan.new_rows,
            vec![ScreenRow {
                line_index: 0,
                wrap_index: 0
            }]
        );
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
        let mut state = ScreenState::new(&mut doc, 80, 4);
        let plan = state.scroll_down(3, &mut doc);

        assert_eq!(plan.terminal_scroll, 3);
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
        let mut state = ScreenState::new(&mut doc, 80, 4);
        // Try to scroll down by 10, but only 1 row available
        let plan = state.scroll_down(10, &mut doc);

        assert_eq!(plan.terminal_scroll, 1);
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
        let mut state = ScreenState::new(&mut doc, 80, 4);
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
        let mut state = ScreenState::new(&mut doc, 80, 4);
        let changed = state.jump_to(&mut doc, 0);
        assert!(!changed);
    }

    #[test]
    fn fewer_lines_than_height() {
        let mut doc = make_doc(&["a", "b"]);
        let state = ScreenState::new(&mut doc, 80, 6);
        assert_eq!(state.rows().len(), 2);
    }

    #[test]
    fn jump_to_end() {
        let mut doc = make_doc(&["a", "b", "c", "d", "e"]);
        let mut state = ScreenState::new(&mut doc, 80, 4);
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
        let mut state = ScreenState::new(&mut doc, 5, 4);
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
        let mut state = ScreenState::new(&mut doc, 80, 6);
        let changed = state.jump_to_end(&mut doc);
        // Already showing everything, no change
        assert!(!changed);
    }
}
