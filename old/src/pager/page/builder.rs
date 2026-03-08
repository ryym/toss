use std::collections::VecDeque;

use crate::pager::{
    PageLine,
    page::{FilledPage, Row},
};

pub(in crate::pager) struct NewPageBuilder {
    deque: VecDeque<PageLine>,
    end_row: Option<Row>,
    row_capacity: usize,
    read_rows: usize,
}

impl NewPageBuilder {
    pub fn new(row_capacity: usize) -> Self {
        debug_assert!(row_capacity > 0);
        Self {
            deque: VecDeque::with_capacity(row_capacity + 1),
            end_row: None,
            row_capacity,
            read_rows: 0,
        }
    }

    pub fn push_back(&mut self, line: PageLine) -> bool {
        debug_assert!(self.read_rows < self.row_capacity);
        match push_back_line(
            line,
            &mut self.deque,
            &mut self.read_rows,
            self.row_capacity,
        ) {
            None => true,
            Some(end_row) => {
                self.end_row = Some(end_row);
                false
            }
        }
    }

    pub fn into_page(mut self) -> Option<FilledPage> {
        if self.deque.is_empty() {
            return None;
        }
        let (start_row, end_row) = finalize_start_page_rows(&self.deque, self.end_row.take());
        Some(FilledPage {
            deque: self.deque,
            row_capacity: self.row_capacity,
            start_row,
            end_row,
        })
    }
}

pub(in crate::pager) struct ForwardPageWriter<'page> {
    page: &'page mut FilledPage,
    end_row: Option<Row>,
    read_rows: usize,
}

impl<'page> ForwardPageWriter<'page> {
    pub fn for_page(page: &'page mut FilledPage) -> Self {
        page.deque.clear();
        Self {
            page,
            end_row: None,
            read_rows: 0,
        }
    }

    pub fn push_back(&mut self, line: PageLine) -> bool {
        debug_assert!(self.read_rows < self.page.row_capacity);

        match push_back_line(
            line,
            &mut self.page.deque,
            &mut self.read_rows,
            self.page.row_capacity,
        ) {
            None => true,
            Some(end_row) => {
                self.end_row = Some(end_row);
                false
            }
        }
    }

    pub fn write_to_page(mut self) {
        let (start_row, end_row) = finalize_start_page_rows(&self.page.deque, self.end_row.take());
        self.page.start_row = start_row;
        self.page.end_row = end_row;
    }
}

fn push_back_line(
    line: PageLine,
    deque: &mut VecDeque<PageLine>,
    read_rows: &mut usize,
    row_capacity: usize,
) -> Option<Row> {
    *read_rows += line.row_len();
    if *read_rows < row_capacity {
        deque.push_back(line);
        return None;
    }

    let end_row = Row {
        deque_index: deque.len(),
        wrap_row_index: line.row_len() - 1 - (*read_rows - row_capacity),
    };
    deque.push_back(line);
    Some(end_row)
}

fn finalize_start_page_rows(deque: &VecDeque<PageLine>, end_row: Option<Row>) -> (Row, Row) {
    let start_row = Row {
        deque_index: 0,
        wrap_row_index: 0,
    };
    // The end row is not set when lines are less than the page size.
    let end_row = end_row.unwrap_or_else(|| Row {
        deque_index: deque.len() - 1,
        wrap_row_index: deque[deque.len() - 1].row_len() - 1,
    });
    (start_row, end_row)
}

pub(in crate::pager) struct BackwardPageWriter<'page> {
    page: &'page mut FilledPage,
    start_row: Option<Row>,
    read_rows: usize,
}

impl<'page> BackwardPageWriter<'page> {
    pub fn for_page(page: &'page mut FilledPage) -> Self {
        page.deque.clear();
        Self {
            page,
            start_row: None,
            read_rows: 0,
        }
    }

    pub fn push_front(&mut self, line: PageLine) -> bool {
        debug_assert!(self.read_rows < self.page.row_capacity);

        self.read_rows += line.row_len();
        if self.read_rows < self.page.row_capacity {
            self.page.deque.push_front(line);
            return true;
        }

        self.start_row = Some(Row {
            deque_index: 0,
            wrap_row_index: self.read_rows - self.page.row_capacity,
        });
        self.page.deque.push_front(line);
        false
    }

    pub fn write_to_page(self) {
        let end_row = Row {
            deque_index: self.page.deque.len() - 1,
            wrap_row_index: self.page.deque[self.page.deque.len() - 1].row_len() - 1,
        };
        // The start row is not set when lines are less than the page size.
        let start_row = self.start_row.unwrap_or(Row {
            deque_index: 0,
            wrap_row_index: 0,
        });
        self.page.start_row = start_row;
        self.page.end_row = end_row;
    }
}
