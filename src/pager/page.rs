use std::collections::VecDeque;

use crate::pager::line::{PageLine, RowSpan};

mod builder;

pub(super) use builder::{EndPageWriter, NewPageBuilder, StartPageWriter};

#[derive(Debug, Default)]
pub(super) struct Row {
    deque_index: usize,
    slice_index: usize,
}

impl PartialEq for Row {
    fn eq(&self, other: &Self) -> bool {
        self.deque_index == other.deque_index && self.slice_index == other.slice_index
    }
}

/// A page which holds text lines in the page.
/// See [`crate::pager::line::PageLine`] for some terminologies.
#[derive(Debug)]
pub(super) struct Page<LineMeta> {
    // We use a double-ended queue to cache lines for now but perhaps it is not a best choice.
    deque: VecDeque<PageLine<LineMeta>>,
    row_size: usize,
    start_row: Row,
    end_row: Row,
}

impl<LineMeta> Page<LineMeta> {
    pub fn builder(row_size: usize) -> NewPageBuilder<LineMeta> {
        NewPageBuilder::new(row_size)
    }

    pub fn start_line(&self) -> &PageLine<LineMeta> {
        &self.deque[self.start_row.deque_index]
    }

    pub fn end_line(&self) -> &PageLine<LineMeta> {
        &self.deque[self.end_row.deque_index]
    }

    pub fn start_row_span(&self) -> RowSpan<'_> {
        let line = &self.deque[self.start_row.deque_index];
        line.slice(self.start_row.slice_index..)
    }

    pub fn end_row_span(&self) -> RowSpan<'_> {
        let line = &self.deque[self.end_row.deque_index];
        line.slice(..=self.end_row.slice_index)
    }

    pub fn row_spans(&self) -> RowSpanIter<'_, LineMeta> {
        RowSpanIter::new(self)
    }

    pub fn push_back(&mut self, line: PageLine<LineMeta>) {
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

    pub fn push_front(&mut self, line: PageLine<LineMeta>) {
        // As long as the dequeue capacity is greater than the page size and this method is called
        // when the page is at the start of the dequeue, the last element of the dequeue must not
        // in the page and therefore it should be safe to remove it.
        debug_assert!(self.start_row.deque_index == 0);
        if self.deque.len() == self.deque.capacity() {
            self.deque.pop_back();
            self.start_row.deque_index += 1;
            self.end_row.deque_index += 1;
        }
        self.deque.push_front(line);
    }

    pub fn move_down_one_row(&mut self) -> bool {
        if !move_down_row(&mut self.deque, &mut self.end_row) {
            return false;
        }
        if self.start_row != self.end_row {
            move_down_row(&mut self.deque, &mut self.start_row);
        }
        true
    }

    pub fn move_up_one_row(&mut self) -> bool {
        if !move_up_row(&mut self.start_row) {
            return false;
        }
        if self.start_row != self.end_row {
            move_up_row(&mut self.end_row);
        }
        true
    }

    pub fn start_page_writer(&mut self) -> StartPageWriter<'_, LineMeta> {
        StartPageWriter::for_page(self)
    }

    pub fn end_page_writer(&mut self) -> EndPageWriter<'_, LineMeta> {
        EndPageWriter::for_page(self)
    }
}

fn move_down_row<LineMeta>(deque: &mut VecDeque<PageLine<LineMeta>>, row: &mut Row) -> bool {
    let line = &deque[row.deque_index];
    if row.slice_index < line.row_len() - 1 {
        row.slice_index += 1;
        true
    } else if deque.get(row.deque_index + 1).is_some() {
        *row = Row {
            deque_index: row.deque_index + 1,
            slice_index: 0,
        };
        true
    } else {
        false
    }
}

fn move_up_row(row: &mut Row) -> bool {
    if 0 < row.slice_index {
        row.slice_index -= 1;
        true
    } else if 0 < row.deque_index {
        *row = Row {
            deque_index: row.deque_index - 1,
            slice_index: 0,
        };
        true
    } else {
        false
    }
}

#[derive(Debug)]
pub(crate) struct RowSpanIter<'page, LineMeta> {
    page: &'page Page<LineMeta>,
    current_deque_index: usize,
}

impl<'page, LineMeta> RowSpanIter<'page, LineMeta> {
    fn new(page: &'page Page<LineMeta>) -> Self {
        Self {
            page,
            current_deque_index: page.start_row.deque_index,
        }
    }
}

impl<'page, LineMeta> Iterator for RowSpanIter<'page, LineMeta> {
    type Item = RowSpan<'page>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.page.deque.get(self.current_deque_index) {
            None => None,
            Some(line) => {
                if self.current_deque_index == self.page.end_row.deque_index {
                    self.current_deque_index = self.page.deque.len();
                    Some(line.slice(..=self.page.end_row.slice_index))
                } else if self.current_deque_index == self.page.start_row.deque_index {
                    self.current_deque_index += 1;
                    Some(line.slice(self.page.start_row.slice_index..))
                } else {
                    self.current_deque_index += 1;
                    Some(line.slice(..))
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use crate::pager::{PageLine, line::RowSpan, page::Page};

    #[test]
    fn hold_lines_less_than_page_size() {
        let mut builder = Page::builder(3);
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
        let mut builder = Page::builder(3);
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

        // Cannot move down row further.
        assert_eq!(page.move_down_one_row(), false);
        assert_eq!(page.row_spans().collect::<Vec<_>>(), initial);

        // But by pushing an additional line,
        page.push_back(PageLine::new((), 'd'.to_string(), 3));
        assert_eq!(page.row_spans().collect::<Vec<_>>(), initial);

        // Now the page can move down.
        assert_eq!(page.move_down_one_row(), true);
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
