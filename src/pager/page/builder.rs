use std::collections::VecDeque;

use crate::pager::{Page, PageLine, page::Row};

pub(in crate::pager) struct NewPageBuilder<LineMeta> {
    deque: VecDeque<PageLine<LineMeta>>,
    end_row: Option<Row>,
    row_size: usize,
    read_rows: usize,
}

impl<LineMeta> NewPageBuilder<LineMeta> {
    pub fn new(row_size: usize) -> Self {
        debug_assert!(row_size > 0);
        Self {
            deque: VecDeque::with_capacity(row_size + 1),
            end_row: None,
            row_size,
            read_rows: 0,
        }
    }

    pub fn push_back(&mut self, line: PageLine<LineMeta>) -> bool {
        debug_assert!(self.read_rows < self.row_size);
        match push_back_line(line, &mut self.deque, &mut self.read_rows, self.row_size) {
            None => true,
            Some(end_row) => {
                self.end_row = Some(end_row);
                false
            }
        }
    }

    pub fn into_page(mut self) -> Option<Page<LineMeta>> {
        if self.deque.is_empty() {
            return None;
        }
        let (start_row, end_row) = finalize_start_page_rows(&self.deque, self.end_row.take());
        Some(Page {
            deque: self.deque,
            row_size: self.row_size,
            start_row,
            end_row,
        })
    }
}

pub(in crate::pager) struct StartPageWriter<'page, LineMeta> {
    page: &'page mut Page<LineMeta>,
    end_row: Option<Row>,
    read_rows: usize,
}

impl<'page, LineMeta> StartPageWriter<'page, LineMeta> {
    pub fn for_page(page: &'page mut Page<LineMeta>) -> Self {
        page.deque.clear();
        Self {
            page,
            end_row: None,
            read_rows: 0,
        }
    }

    pub fn push_back(&mut self, line: PageLine<LineMeta>) -> bool {
        debug_assert!(self.read_rows < self.page.row_size);

        match push_back_line(
            line,
            &mut self.page.deque,
            &mut self.read_rows,
            self.page.row_size,
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

fn push_back_line<LineMeta>(
    line: PageLine<LineMeta>,
    deque: &mut VecDeque<PageLine<LineMeta>>,
    read_rows: &mut usize,
    row_size: usize,
) -> Option<Row> {
    *read_rows += line.row_len();
    if *read_rows < row_size {
        deque.push_back(line);
        return None;
    }

    let end_slice_idx = line.row_len() - 1 - (*read_rows - row_size);
    let end_row = Row {
        deque_index: deque.len(),
        slice_index: end_slice_idx,
    };
    deque.push_back(line);
    Some(end_row)
}

fn finalize_start_page_rows<LineMeta>(
    deque: &VecDeque<PageLine<LineMeta>>,
    end_row: Option<Row>,
) -> (Row, Row) {
    let start_row = Row {
        deque_index: 0,
        slice_index: 0,
    };
    // The end row is not set when lines are less than the page size.
    let end_row = end_row.unwrap_or_else(|| Row {
        deque_index: deque.len() - 1,
        slice_index: deque[deque.len() - 1].row_len() - 1,
    });
    (start_row, end_row)
}

pub(in crate::pager) struct EndPageWriter<'page, LineMeta> {
    page: &'page mut Page<LineMeta>,
    start_row: Option<Row>,
    read_rows: usize,
}

impl<'page, LineMeta> EndPageWriter<'page, LineMeta> {
    pub fn for_page(page: &'page mut Page<LineMeta>) -> Self {
        page.deque.clear();
        Self {
            page,
            start_row: None,
            read_rows: 0,
        }
    }

    pub fn push_front(&mut self, line: PageLine<LineMeta>) -> bool {
        debug_assert!(self.read_rows < self.page.row_size);

        self.read_rows += line.row_len();
        if self.read_rows < self.page.row_size {
            self.page.deque.push_front(line);
            return true;
        }

        let start_slice_idx = self.read_rows - self.page.row_size;
        self.start_row = Some(Row {
            deque_index: 0,
            slice_index: start_slice_idx,
        });
        self.page.deque.push_front(line);
        false
    }

    pub fn write_to_page(self) {
        let end_row = Row {
            deque_index: self.page.deque.len() - 1,
            slice_index: self.page.deque[self.page.deque.len() - 1].row_len() - 1,
        };
        // The start row is not set when lines are less than the page size.
        let start_row = self.start_row.unwrap_or(Row {
            deque_index: 0,
            slice_index: 0,
        });
        self.page.start_row = start_row;
        self.page.end_row = end_row;
    }
}
