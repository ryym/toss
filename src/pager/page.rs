use std::collections::VecDeque;

use regex::Regex;

use crate::pager::{
    line::{PageLine, RowSpan},
    page::builder::{BackwardPageWriter, ForwardPageWriter, NewPageBuilder},
};

mod builder;

/// The start or end row of the page.
#[derive(Debug, PartialEq)]
struct Row {
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

/// A page which holds text lines in the page.
/// [`crate::pager::Pager`] loads line from the source text and stores them in the page.
/// See [`PageLine`] for some terminologies.
#[derive(Debug)]
pub(super) struct FilledPage<LineMeta> {
    /// A double ended queue that holds lines currently displayed in the page.
    deque: VecDeque<PageLine<LineMeta>>,
    row_size: usize,
    start_row: Row,
    end_row: Row,
}

impl<LineMeta> FilledPage<LineMeta> {
    pub fn builder(row_size: usize) -> NewPageBuilder<LineMeta> {
        NewPageBuilder::new(row_size)
    }

    pub fn start_line(&self) -> &PageLine<LineMeta> {
        &self.deque[0]
    }

    pub fn end_line(&self) -> &PageLine<LineMeta> {
        &self.deque[self.deque.len() - 1]
    }

    pub fn start_row_span(&self) -> RowSpan<'_> {
        self.start_line().slice(self.start_row.wrap_row_index..)
    }

    pub fn end_row_span(&self) -> RowSpan<'_> {
        self.end_line().slice(..=self.end_row.wrap_row_index)
    }

    pub fn row_spans(&self) -> RowSpanIter<'_, LineMeta> {
        RowSpanIter::from_page(self)
    }

    /// Try to move down the page one row without loading a new line.
    /// This succeeds only when the bottom line has more wrap rows which is not in the page.
    pub fn move_down_one_row(&mut self) -> bool {
        if !move_down_row(&self.deque[self.deque.len() - 1], &mut self.end_row) {
            return false;
        }
        if !move_down_row(&self.deque[0], &mut self.start_row) {
            self.deque.pop_front();
        }
        true
    }

    /// Try to move up the page one row without loading a new line.
    /// This succeeds only when the top line has more wrap rows which is not in the page.
    pub fn move_up_one_row(&mut self) -> bool {
        if !move_up_row(&mut self.start_row) {
            return false;
        }
        if !move_up_row(&mut self.end_row) {
            self.deque.pop_back();
        }
        true
    }

    /// Move down to the next row by addig a new line.
    pub fn move_down_one_line(&mut self, line: PageLine<LineMeta>) {
        self.deque.pop_front();
        self.deque.push_back(line);
    }

    /// Move up to the next row by addig a new line.
    pub fn move_up_one_line(&mut self, line: PageLine<LineMeta>) {
        self.deque.pop_back();
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
            .find(|line| search_query.is_match(line.plain()))
    }
}

fn move_down_row<LineMeta>(line: &PageLine<LineMeta>, row: &mut Row) -> bool {
    if row.wrap_row_index < line.row_len() - 1 {
        row.wrap_row_index += 1;
        true
    } else {
        false
    }
}

fn move_up_row(row: &mut Row) -> bool {
    if 0 < row.wrap_row_index {
        row.wrap_row_index -= 1;
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
            dummy_row: Row { wrap_row_index: 0 },
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
            deque_index: 0,
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
                let row_span = if self.deque_index == 0 {
                    line.slice(self.start_row.wrap_row_index..)
                } else if self.deque_index == self.deque.len() - 1 {
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
