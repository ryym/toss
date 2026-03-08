use std::{
    collections::VecDeque,
    fmt::Debug,
    ops::{Bound, RangeBounds},
};

use regex::Regex;

use crate::{
    pager::{
        line::{LineSlice, PageLine},
        page::builder::{BackwardPageWriter, ForwardPageWriter, NewPageBuilder},
    },
    reader::LinePos,
};

mod builder;

/// The start or end row of the page.
#[derive(Debug, Clone, PartialEq)]
struct Row {
    /// The index of the row in [`FilledPage::deque`].
    /// Note that this index has nothing to do with the line position in the source text.
    deque_index: usize,
    /// A wrap row index of this `Row`. See [`PageLine`] for terminologies. For example,
    /// if this is the start row of the page and the page is positioned like below,
    /// ```text
    ///  ┌──────────────────────────────┐ Entire text
    ///  │  A long line is wrapped aut  │
    ///  │┌ - - - - - - - - - - - - - ┐ Current page
    ///  │❘ omatically like this. ---------> this `Row`
    ///  │❘ This is another line.     ❘ │
    ///  │❘ ...                       ❘ │
    /// ```
    /// The line is wrapped into 2 rows and the `wrap_row_index` is 1.
    wrap_row_index: usize,
}

/// FilledPage manages rows currently displayed in the page.
/// If a line is too long to fit in the page width, it is wrapped into multiple rows.
/// This struct provides easy-to-use methods by hiding the complexity of line wraps,
/// but this struct itself does not load lines from source. Lines are set by [`crate::pager::Pager`].
/// See [`PageLine`] for detailed terminologies like line v.s. row.
pub(super) struct FilledPage {
    /// A double ended queue that holds lines currently displayed in the page, and
    /// additional one or more lines before and after the displayed lines.
    /// Additional lines work as a cache but its main purpose is to simplify the implementation.
    deque: VecDeque<PageLine>,
    row_capacity: usize,
    start_row: Row,
    end_row: Row,
}

impl Debug for FilledPage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FilledPage")
            .field("lines_len", &self.deque.len())
            .field("start_row", &self.start_row)
            .field("end_row", &self.end_row)
            .field("start_line_pos", &self.start_line().pos())
            .field("end_line_pos", &self.end_line().pos())
            .finish()
    }
}

impl FilledPage {
    pub fn builder(row_capacity: usize) -> NewPageBuilder {
        NewPageBuilder::new(row_capacity)
    }

    pub fn start_line(&self) -> &PageLine {
        &self.deque[self.start_row.deque_index]
    }

    pub fn end_line(&self) -> &PageLine {
        &self.deque[self.end_row.deque_index]
    }

    pub fn start_line_slice(&self) -> LineSlice<'_> {
        let line = &self.deque[self.start_row.deque_index];
        line.slice(self.start_row.wrap_row_index..)
    }

    pub fn end_line_slice(&self) -> LineSlice<'_> {
        let line = &self.deque[self.end_row.deque_index];
        line.slice(..=self.end_row.wrap_row_index)
    }

    pub fn line_slices(&self, row_range: impl RangeBounds<usize>) -> LineSliceIter<'_> {
        let start_row_idx = match row_range.start_bound() {
            Bound::Unbounded => 0,
            Bound::Included(i) => *i,
            Bound::Excluded(i) => *i + 1,
        };
        let end_row_idx = match row_range.end_bound() {
            Bound::Unbounded => self.row_len() - 1,
            Bound::Included(i) => *i,
            Bound::Excluded(i) => {
                assert!(*i > 0, "excluded bound must be greater than 0");
                i - 1
            }
        };
        let start_row = self.row_index_to_row(start_row_idx);
        let end_row = self.row_index_to_row(end_row_idx);
        LineSliceIter::from_page(self, start_row, end_row)
    }

    fn row_index_to_row(&self, row_idx: usize) -> Row {
        let mut current_row_idx = 0;
        let mut deque_idx = self.start_row.deque_index;
        let mut wrap_idx = self.start_row.wrap_row_index;

        while deque_idx <= self.end_row.deque_index {
            let line = &self.deque[deque_idx];
            let available_wraps = if deque_idx == self.start_row.deque_index {
                if deque_idx == self.end_row.deque_index {
                    self.end_row.wrap_row_index - wrap_idx + 1
                } else {
                    line.row_len() - wrap_idx
                }
            } else if deque_idx == self.end_row.deque_index {
                self.end_row.wrap_row_index + 1
            } else {
                line.row_len()
            };

            if current_row_idx + available_wraps > row_idx {
                let offset = row_idx - current_row_idx;
                return Row {
                    deque_index: deque_idx,
                    wrap_row_index: wrap_idx + offset,
                };
            }

            current_row_idx += available_wraps;
            deque_idx += 1;
            wrap_idx = 0;
        }

        panic!("row index {row_idx} too large")
    }

    pub fn row_len(&self) -> usize {
        self.row_len_of_lines(..)
    }

    fn row_len_of_lines(&self, deque_range: impl RangeBounds<usize>) -> usize {
        let start_idx = match deque_range.start_bound() {
            Bound::Unbounded => self.start_row.deque_index,
            Bound::Included(i) => *i,
            Bound::Excluded(_) => panic!("unexpected excluded bound for page rows range"),
        };
        let end = match deque_range.end_bound() {
            Bound::Unbounded => self.end_row.deque_index + 1,
            Bound::Included(i) => *i + 1,
            Bound::Excluded(i) => *i,
        };
        debug_assert!(start_idx <= self.start_row.deque_index);
        debug_assert!(end <= self.end_row.deque_index + 1);

        // Sum up the number of rows of lines in the given range.
        let mut row_len = self
            .deque
            .iter()
            .take(end)
            .skip(start_idx)
            .fold(0, |n, line| n + line.row_len());

        // Subtract the number of unvisible rows if they are included.
        if start_idx == self.start_row.deque_index {
            row_len -= self.start_row.wrap_row_index;
        }
        if end == self.end_row.deque_index + 1 {
            row_len -= self.end_line().row_len() - 1 - self.end_row.wrap_row_index;
        }

        row_len
    }

    pub fn can_move_down_one_row(&self) -> bool {
        self.end_row.wrap_row_index < self.end_line().row_len() - 1
            || self.end_row.deque_index < self.deque.len() - 1
    }

    pub fn can_move_up_one_row(&self) -> bool {
        0 < self.start_row.wrap_row_index || 0 < self.start_row.deque_index
    }

    /// Try to move down the page one row without loading a new line. This succeeds in either case:
    /// - when the bottom line has more wrap rows which is not in the page.
    /// - when the page has additional cached lines which has been previously loaded.
    pub fn move_down_one_row(&mut self) -> bool {
        let row_len = self.row_len();
        if !move_down_row(&self.deque, &mut self.end_row) {
            return false;
        }
        if row_len == self.row_capacity {
            move_down_row(&self.deque, &mut self.start_row);
        }
        true
    }

    /// Try to move up the page one row without loading a new line. This succeeds in either case:
    /// - when the top line has more wrap rows which is not in the page.
    /// - when the page has additional cached lines which has been previously loaded.
    pub fn move_up_one_row(&mut self) -> bool {
        let row_len = self.row_len();
        if !move_up_row(&self.deque, &mut self.start_row) {
            return false;
        }
        if row_len == self.row_capacity {
            move_up_row(&self.deque, &mut self.end_row);
        }
        true
    }

    /// Move down to the next row by addig a new line.
    pub fn move_down_one_line(&mut self, line: PageLine) {
        self.push_back(line);
        self.move_down_one_row();
    }

    /// Move up to the next row by addig a new line.
    pub fn move_up_one_line(&mut self, line: PageLine) {
        self.push_front(line);
        self.move_up_one_row();
    }

    fn push_back(&mut self, line: PageLine) {
        // As long as the dequeue capacity is greater than the page size and this method is called
        // when the page is at the end of the dequeue, the first element of the dequeue must not
        // in the page and therefore it should be safe to remove it.
        debug_assert!(self.end_row.deque_index == self.deque.len() - 1);
        if self.deque.len() == self.deque.capacity() {
            self.deque.pop_front();
            self.start_row.deque_index -= 1;
            self.end_row.deque_index -= 1;
        }
        self.deque.push_back(line);
    }

    fn push_front(&mut self, line: PageLine) {
        // As long as the dequeue capacity is greater than the page size and this method is called
        // when the page is at the start of the dequeue, the last element of the dequeue must not
        // in the page and therefore it should be safe to remove it.
        debug_assert!(self.start_row.deque_index == 0);
        if self.deque.len() == self.deque.capacity() {
            self.deque.pop_back();
        }
        self.start_row.deque_index += 1;
        self.end_row.deque_index += 1;
        self.deque.push_front(line);
    }

    pub fn forward_page_writer(&mut self) -> ForwardPageWriter<'_> {
        ForwardPageWriter::for_page(self)
    }

    pub fn backward_page_writer(&mut self) -> BackwardPageWriter<'_> {
        BackwardPageWriter::for_page(self)
    }

    pub fn find_first_match_line(&self, query: &Regex) -> Option<MatchingLine> {
        self.find_first_match_after(0, query)
    }

    pub fn find_second_match_line(&self, query: &Regex) -> Option<MatchingLine> {
        self.find_first_match_after(1, query)
    }

    fn find_first_match_after(&self, skip: usize, query: &Regex) -> Option<MatchingLine> {
        let skipped = self.start_row.deque_index + skip;
        self.deque
            .iter()
            .skip(skipped)
            .position(|line| query.is_match(line.plain()))
            .map(|idx| {
                let line = &self.deque[skipped + idx];
                MatchingLine {
                    passed_rows: self.row_len_of_lines(..skipped + idx),
                    pos: *line.pos(),
                }
            })
    }

    pub fn find_last_match_line(&self, query: &Regex) -> Option<&PageLine> {
        let overflow_rows = self.deque.len() - self.end_row.deque_index - 1;
        self.deque
            .iter()
            .rev()
            .skip(overflow_rows)
            .find(|line| query.is_match(line.plain()))
    }
}

fn move_down_row(deque: &VecDeque<PageLine>, row: &mut Row) -> bool {
    let line = &deque[row.deque_index];
    if row.wrap_row_index < line.row_len() - 1 {
        row.wrap_row_index += 1;
        true
    } else if deque.get(row.deque_index + 1).is_some() {
        *row = Row {
            deque_index: row.deque_index + 1,
            wrap_row_index: 0,
        };
        true
    } else {
        false
    }
}

fn move_up_row(deque: &VecDeque<PageLine>, row: &mut Row) -> bool {
    if 0 < row.wrap_row_index {
        row.wrap_row_index -= 1;
        true
    } else if 0 < row.deque_index {
        *row = Row {
            deque_index: row.deque_index - 1,
            wrap_row_index: deque[row.deque_index - 1].row_len() - 1,
        };
        true
    } else {
        false
    }
}

#[derive(Debug)]
pub(super) struct MatchingLine {
    /// The number of rows passed through from the last line position to the matching line.
    passed_rows: usize,
    pos: LinePos,
}

impl MatchingLine {
    #[inline]
    pub fn passed_rows(&self) -> usize {
        self.passed_rows
    }

    #[inline]
    pub fn pos(&self) -> &LinePos {
        &self.pos
    }
}

#[derive(Debug)]
pub(super) struct EmptyPage {
    deque: VecDeque<PageLine>,
    dummy_row: Row,
}

impl EmptyPage {
    pub fn new() -> Self {
        Self {
            deque: VecDeque::new(),
            dummy_row: Row {
                deque_index: 0,
                wrap_row_index: 0,
            },
        }
    }

    pub fn line_slices(&self) -> LineSliceIter<'_> {
        LineSliceIter::empty(self)
    }
}

#[derive(Debug)]
pub(crate) struct LineSliceIter<'page> {
    deque: &'page VecDeque<PageLine>,
    start_row: Row,
    end_row: Row,
    deque_index: usize,
}

impl<'page> LineSliceIter<'page> {
    fn from_page(page: &'page FilledPage, start_row: Row, end_row: Row) -> Self {
        Self {
            deque: &page.deque,
            deque_index: start_row.deque_index,
            start_row,
            end_row,
        }
    }

    fn empty(page: &'page EmptyPage) -> Self {
        Self {
            deque: &page.deque,
            start_row: page.dummy_row.clone(),
            end_row: page.dummy_row.clone(),
            deque_index: 0,
        }
    }

    #[cfg(test)]
    pub fn into_vec(self) -> Vec<(String, usize)> {
        self.map(|l| (l.to_string(), l.row_len())).collect()
    }
}

impl<'page> Iterator for LineSliceIter<'page> {
    type Item = LineSlice<'page>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.deque_index > self.end_row.deque_index {
            return None;
        }

        match self.deque.get(self.deque_index) {
            None => None,
            Some(line) => {
                let line_slice = if self.deque_index == self.start_row.deque_index {
                    if self.deque_index == self.end_row.deque_index {
                        line.slice(self.start_row.wrap_row_index..=self.end_row.wrap_row_index)
                    } else {
                        line.slice(self.start_row.wrap_row_index..)
                    }
                } else if self.deque_index == self.end_row.deque_index {
                    line.slice(..=self.end_row.wrap_row_index)
                } else {
                    line.slice(..)
                };
                self.deque_index += 1;
                Some(line_slice)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use crate::pager::{PageLine, page::FilledPage};

    #[test]
    fn hold_lines_less_than_page_size() {
        let mut builder = FilledPage::builder(3);
        builder.push_back(PageLine::dummy("abc".to_string(), 3));
        builder.push_back(PageLine::dummy("def".to_string(), 3));
        let page = builder.into_page().expect("build page");
        assert_eq!(
            page.line_slices(..).into_vec(),
            vec![("abc".to_string(), 1), ("def".to_string(), 1)]
        );
    }

    #[test]
    fn move_page_across_lines() {
        let mut builder = FilledPage::builder(3);
        for chr in 'a'..='c' {
            builder.push_back(PageLine::dummy(chr.to_string(), 3));
        }
        let mut page = builder.into_page().expect("build page");
        assert_eq!(
            page.line_slices(..).into_vec(),
            vec![
                ("a".to_string(), 1),
                ("b".to_string(), 1),
                ("c".to_string(), 1)
            ]
        );

        assert_eq!(page.move_down_one_row(), false);
        assert_eq!(
            page.line_slices(..).into_vec(),
            vec![
                ("a".to_string(), 1),
                ("b".to_string(), 1),
                ("c".to_string(), 1)
            ]
        );

        page.move_down_one_line(PageLine::dummy('d'.to_string(), 3));
        assert_eq!(
            page.line_slices(..).into_vec(),
            vec![
                ("b".to_string(), 1),
                ("c".to_string(), 1),
                ("d".to_string(), 1)
            ]
        );
    }
}
