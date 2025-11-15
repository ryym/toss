use std::{collections::VecDeque, fmt::Debug};

use regex::Regex;

use crate::pager::{
    line::{PageLine, RowSpan},
    page::builder::{BackwardPageWriter, ForwardPageWriter, NewPageBuilder},
};

mod builder;

/// The start or end row of the page.
#[derive(Debug, PartialEq)]
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
pub(super) struct FilledPage<LineMeta> {
    /// A double ended queue that holds lines currently displayed in the page, and
    /// additional one or more lines before and after the displayed lines.
    /// Additional lines work as a cache but its main purpose is to simplify the implementation.
    deque: VecDeque<PageLine<LineMeta>>,
    row_capacity: usize,
    start_row: Row,
    end_row: Row,
}

impl<LineMeta: Debug> Debug for FilledPage<LineMeta> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FilledPage")
            .field("lines_len", &self.deque.len())
            .field("start_row", &self.start_row)
            .field("end_row", &self.end_row)
            .field("start_line_meta", &self.start_line().meta())
            .field("end_line_meta", &self.end_line().meta())
            .finish()
    }
}

impl<LineMeta> FilledPage<LineMeta> {
    pub fn builder(row_capacity: usize) -> NewPageBuilder<LineMeta> {
        NewPageBuilder::new(row_capacity)
    }

    pub fn start_line(&self) -> &PageLine<LineMeta> {
        &self.deque[self.start_row.deque_index]
    }

    pub fn end_line(&self) -> &PageLine<LineMeta> {
        &self.deque[self.end_row.deque_index]
    }

    pub fn start_row_span(&self) -> RowSpan<'_> {
        let line = &self.deque[self.start_row.deque_index];
        line.slice(self.start_row.wrap_row_index..)
    }

    pub fn end_row_span(&self) -> RowSpan<'_> {
        let line = &self.deque[self.end_row.deque_index];
        line.slice(..=self.end_row.wrap_row_index)
    }

    pub fn row_spans(&self) -> RowSpanIter<'_, LineMeta> {
        RowSpanIter::from_page(self)
    }

    fn visible_lines(&self) -> impl Iterator<Item = &PageLine<LineMeta>> {
        self.deque
            .iter()
            .take(self.end_row.deque_index + 1)
            .skip(self.start_row.deque_index)
    }

    fn row_len(&self) -> usize {
        let visible_line_rows = self.visible_lines().fold(0, |n, line| n + line.row_len());
        let unvisible_rows = self.start_row.wrap_row_index
            + (self.end_line().row_len() - 1 - self.end_row.wrap_row_index);
        visible_line_rows - unvisible_rows
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
    pub fn move_down_one_line(&mut self, line: PageLine<LineMeta>) {
        self.push_back(line);
        self.move_down_one_row();
    }

    /// Move up to the next row by addig a new line.
    pub fn move_up_one_line(&mut self, line: PageLine<LineMeta>) {
        self.push_front(line);
        self.move_up_one_row();
    }

    fn push_back(&mut self, line: PageLine<LineMeta>) {
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

    fn push_front(&mut self, line: PageLine<LineMeta>) {
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

    pub fn forward_page_writer(&mut self) -> ForwardPageWriter<'_, LineMeta> {
        ForwardPageWriter::for_page(self)
    }

    pub fn backward_page_writer(&mut self) -> BackwardPageWriter<'_, LineMeta> {
        BackwardPageWriter::for_page(self)
    }

    pub fn find_first_match_line(&mut self, search_query: &Regex) -> Option<&PageLine<LineMeta>> {
        self.deque
            .iter()
            .skip(self.start_row.deque_index)
            .find(|line| search_query.is_match(line.plain()))
    }
}

fn move_down_row<LineMeta>(deque: &VecDeque<PageLine<LineMeta>>, row: &mut Row) -> bool {
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

fn move_up_row<LineMeta>(deque: &VecDeque<PageLine<LineMeta>>, row: &mut Row) -> bool {
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
pub(super) struct EmptyPage<LineMeta> {
    deque: VecDeque<PageLine<LineMeta>>,
    dummy_row: Row,
}

impl<LineMeta> EmptyPage<LineMeta> {
    pub fn new() -> Self {
        Self {
            deque: VecDeque::new(),
            dummy_row: Row {
                deque_index: 0,
                wrap_row_index: 0,
            },
        }
    }

    pub fn row_spans(&self) -> RowSpanIter<'_, LineMeta> {
        RowSpanIter::empty(self)
    }
}

#[derive(Debug)]
pub(crate) struct RowSpanIter<'page, LineMeta> {
    deque: &'page VecDeque<PageLine<LineMeta>>,
    start_row: &'page Row,
    end_row: &'page Row,
    deque_index: usize,
}

impl<'page, LineMeta> RowSpanIter<'page, LineMeta> {
    fn from_page(page: &'page FilledPage<LineMeta>) -> Self {
        Self {
            deque: &page.deque,
            start_row: &page.start_row,
            end_row: &page.end_row,
            deque_index: page.start_row.deque_index,
        }
    }

    fn empty(page: &'page EmptyPage<LineMeta>) -> Self {
        Self {
            deque: &page.deque,
            start_row: &page.dummy_row,
            end_row: &page.dummy_row,
            deque_index: 0,
        }
    }
}

impl<'page, LineMeta> Iterator for RowSpanIter<'page, LineMeta> {
    type Item = RowSpan<'page>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.deque.get(self.deque_index) {
            None => None,
            Some(line) => {
                let row_span = if self.deque_index == self.start_row.deque_index {
                    line.slice(self.start_row.wrap_row_index..)
                } else if self.deque_index == self.end_row.deque_index {
                    line.slice(..=self.end_row.wrap_row_index)
                } else {
                    line.slice(..)
                };
                self.deque_index += 1;
                Some(row_span)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use crate::pager::{PageLine, line::RowSpan, page::FilledPage};

    #[test]
    fn hold_lines_less_than_page_size() {
        let mut builder = FilledPage::builder(3);
        builder.push_back(PageLine::new((), "abc".to_string(), 3));
        builder.push_back(PageLine::new((), "def".to_string(), 3));
        let page = builder.into_page().expect("build page");
        assert_eq!(
            page.row_spans().collect::<Vec<_>>(),
            vec![RowSpan::new("abc", 1), RowSpan::new("def", 1)]
        );
    }

    #[test]
    fn move_page_across_lines() {
        let mut builder = FilledPage::builder(3);
        for chr in 'a'..='c' {
            builder.push_back(PageLine::new((), chr.to_string(), 3));
        }
        let mut page = builder.into_page().expect("build page");
        let initial = vec![
            RowSpan::new("a", 1),
            RowSpan::new("b", 1),
            RowSpan::new("c", 1),
        ];
        assert_eq!(page.row_spans().collect::<Vec<_>>(), initial);

        assert_eq!(page.move_down_one_row(), false);
        assert_eq!(page.row_spans().collect::<Vec<_>>(), initial);

        page.move_down_one_line(PageLine::new((), 'd'.to_string(), 3));
        assert_eq!(
            page.row_spans().collect::<Vec<_>>(),
            vec![
                RowSpan::new("b", 1),
                RowSpan::new("c", 1),
                RowSpan::new("d", 1),
            ]
        );
    }
}
