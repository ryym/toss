use crate::{
    error::AnyError,
    pager::{
        line::{PageLine, RowSpan},
        line_store::{Line, LineStore},
    },
    reader::{LinePos, QueryLine, Reader},
    source::Source,
};

mod line;
mod line_store;

#[derive(Debug)]
pub(crate) struct PageSize {
    rows: usize,
    cols: usize,
}

impl PageSize {
    pub fn new(rows: usize, cols: usize) -> Self {
        Self { rows, cols }
    }

    #[inline]
    pub fn rows(&self) -> usize {
        self.rows
    }

    #[inline]
    pub fn cols(&self) -> usize {
        self.cols
    }
}

#[derive(Debug, Clone, PartialEq)]
struct Row {
    line_pos: LinePos,
    slice_index: usize,
    end_slice_index: usize,
}

impl Row {
    fn from_line(line: &Line, slice_index: usize) -> Self {
        Self {
            line_pos: line.meta().pos(),
            slice_index,
            end_slice_index: line.row_len() - 1,
        }
    }
}

/// Pager clips text lines to fit them in the sized frame.
/// Its responsibilites are:
/// 1. Read source text and load to a page.
/// 2. Determine lines that are currently "visible" in the page.
/// 3. Wrap lines based on the column size.
#[derive(Debug)]
pub(crate) struct Pager<R, Src> {
    line_store: LineStore<R, Src>,
    size: PageSize,
    start_row: Option<Row>,
    end_row: Option<Row>,
}

impl<R, Src: Source<R>> Pager<R, Src> {
    pub fn new(reader: Reader<R, Src>, size: PageSize) -> Self {
        Self {
            line_store: LineStore::new(reader, &size),
            size,
            start_row: None,
            end_row: None,
        }
    }

    #[inline]
    pub fn size(&self) -> &PageSize {
        &self.size
    }

    pub fn scroll_down_one_row(&mut self) -> Result<Option<RowSpan<'_>>, AnyError> {
        if !move_down_row(&mut self.line_store, &mut self.end_row)? {
            return Ok(None);
        }
        if self.start_row != self.end_row {
            move_down_row(&mut self.line_store, &mut self.start_row)?;
        }
        self.end_row_span()
    }

    fn end_row_span(&mut self) -> Result<Option<RowSpan<'_>>, AnyError> {
        let end_row = match &self.end_row {
            None => return Ok(None),
            Some(row) => row,
        };
        let query = QueryLine::At(end_row.line_pos);
        let row_span = match self.line_store.read_line(query)? {
            None => None,
            Some(line) => {
                let row_span = line.slice(..=end_row.slice_index);
                Some(row_span)
            }
        };
        Ok(row_span)
    }

    pub fn scroll_up_one_row(&mut self) -> Result<Option<RowSpan<'_>>, AnyError> {
        if !move_up_row(&mut self.line_store, &mut self.start_row)? {
            return Ok(None);
        }
        if self.start_row != self.end_row {
            move_up_row(&mut self.line_store, &mut self.end_row)?;
        }
        self.start_row_span()
    }

    fn start_row_span(&mut self) -> Result<Option<RowSpan<'_>>, AnyError> {
        let start_row = match &self.start_row {
            None => return Ok(None),
            Some(row) => row,
        };
        let query = QueryLine::At(start_row.line_pos);
        let row_span = match self.line_store.read_line(query)? {
            None => None,
            Some(line) => {
                let row_span = line.slice(start_row.slice_index..);
                Some(row_span)
            }
        };
        Ok(row_span)
    }

    pub fn scroll_to_start(&mut self) -> Result<bool, AnyError> {
        if let Some(start_row) = &self.start_row
            && start_row.line_pos.start_byte() == 0
        {
            return Ok(false);
        }
        self.start_row = None;
        self.end_row = None;
        Ok(true)
    }

    pub fn scroll_to_end(&mut self) -> Result<bool, AnyError> {
        let end_row = self.end_row.clone();
        PageLoader::backward(self, QueryLine::AtEnd).run()?;
        Ok(self.end_row != end_row)
    }

    pub fn page(&mut self) -> PageLoader<'_, R, Src> {
        PageLoader::forward_from_current(self)
    }
}

fn move_down_row<R, Src: Source<R>>(
    line_store: &mut LineStore<R, Src>,
    row: &mut Option<Row>,
) -> Result<bool, AnyError> {
    let row = match row {
        None => return Ok(false),
        Some(row) => row,
    };
    if row.slice_index < row.end_slice_index {
        row.slice_index += 1;
        return Ok(true);
    }
    let query = QueryLine::NextOf(row.line_pos);
    match line_store.read_line(query)? {
        None => Ok(false),
        Some(line) => {
            *row = Row::from_line(line, 0);
            Ok(true)
        }
    }
}

fn move_up_row<R, Src: Source<R>>(
    line_store: &mut LineStore<R, Src>,
    row: &mut Option<Row>,
) -> Result<bool, AnyError> {
    let row = match row {
        None => return Ok(false),
        Some(row) => row,
    };
    if 0 < row.slice_index {
        row.slice_index -= 1;
        return Ok(true);
    }
    let query = QueryLine::PrevOf(row.line_pos);
    match line_store.read_line(query)? {
        None => Ok(false),
        Some(line) => {
            *row = Row::from_line(line, line.row_len() - 1);
            Ok(true)
        }
    }
}

pub(crate) struct PageLoader<'p, R, Src> {
    start_row: Option<Row>,
    end_row: Option<Row>,
    read_rows: usize,
    query: QueryLine,
    go_forward: bool,
    pager: &'p mut Pager<R, Src>,
}

impl<'p, R, Src> Drop for PageLoader<'p, R, Src> {
    fn drop(&mut self) {
        if self.start_row.is_some() && self.end_row.is_some() {
            self.pager.start_row = self.start_row.take();
            self.pager.end_row = self.end_row.take();
        }
    }
}

impl<'p, R, Src: Source<R>> PageLoader<'p, R, Src> {
    fn forward_from_current(pager: &'p mut Pager<R, Src>) -> Self {
        let query = match &pager.start_row {
            None => QueryLine::AtStart,
            Some(row) => QueryLine::At(row.line_pos),
        };
        Self {
            read_rows: 0,
            start_row: pager.start_row.clone(),
            end_row: None,
            query,
            go_forward: true,
            pager,
        }
    }

    fn backward(pager: &'p mut Pager<R, Src>, from: QueryLine) -> Self {
        Self {
            read_rows: 0,
            query: from,
            start_row: None,
            end_row: None,
            go_forward: false,
            pager,
        }
    }

    pub fn next_row_span(&mut self) -> Result<Option<RowSpan<'_>>, AnyError> {
        if self.go_forward {
            self.next_forward()
        } else {
            self.next_backward()
        }
    }

    pub fn next_forward(&mut self) -> Result<Option<RowSpan<'_>>, AnyError> {
        let row_size = self.pager.size().rows();
        if self.read_rows >= row_size {
            return Ok(None);
        }
        match self.pager.line_store.read_line(self.query)? {
            None => Ok(None),
            Some(line) => {
                let start_slice_idx = if self.read_rows > 0 {
                    0
                } else {
                    match &self.start_row {
                        Some(row) => row.slice_index,
                        None => {
                            self.start_row = Some(Row::from_line(line, 0));
                            0
                        }
                    }
                };
                self.read_rows += line.row_len() - start_slice_idx;
                let (end_slice_idx, line) = if self.read_rows < row_size {
                    (line.row_len() - 1, line)
                } else {
                    let end_slice_idx = line.row_len() - 1 - (self.read_rows - row_size);
                    self.end_row = Some(Row::from_line(line, end_slice_idx));
                    (end_slice_idx, line)
                };
                self.query = QueryLine::NextOf(line.meta().pos());
                Ok(Some(line.slice(start_slice_idx..=end_slice_idx)))
            }
        }
    }

    fn next_backward(&mut self) -> Result<Option<RowSpan<'_>>, AnyError> {
        let row_size = self.pager.size().rows();
        if self.read_rows >= row_size {
            return Ok(None);
        }
        match self.pager.line_store.read_line(self.query)? {
            None => Ok(None),
            Some(line) => {
                let end_slice_idx = if self.read_rows > 0 {
                    line.row_len() - 1
                } else {
                    match &self.start_row {
                        Some(row) => row.slice_index,
                        None => {
                            let slice_index = line.row_len() - 1;
                            self.end_row = Some(Row::from_line(line, slice_index));
                            slice_index
                        }
                    }
                };
                self.read_rows += line.row_len();
                let (start_slice_idx, line) = if self.read_rows < row_size {
                    (0, line)
                } else {
                    let start_slice_idx = self.read_rows - row_size;
                    self.start_row = Some(Row::from_line(line, start_slice_idx));
                    (start_slice_idx, line)
                };
                self.query = QueryLine::PrevOf(line.meta().pos());
                Ok(Some(line.slice(start_slice_idx..=end_slice_idx)))
            }
        }
    }

    fn run(mut self) -> Result<(), AnyError> {
        while self.next_row_span()?.is_some() {}
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use pretty_assertions::assert_eq;

    use crate::{
        error::AnyError,
        pager::{PageLoader, PageSize, Pager, line::RowSpan},
        reader::Reader,
        source::{self, Source},
    };

    fn page_to_vec<'p, R, Src: Source<R>>(
        loader: &mut PageLoader<'p, R, Src>,
    ) -> Result<Vec<(String, usize)>, AnyError> {
        let mut vec = Vec::new();
        while let Some(row_span) = loader.next_row_span()? {
            vec.push((row_span.line().to_string(), row_span.size()));
        }
        Ok(vec)
    }

    #[test]
    fn pager_scroll_up_down() -> Result<(), AnyError> {
        let s = "abcde\n1234567\nfoo\nbar\n123456789".to_string();
        let cursor = Cursor::new(s);
        let source = source::as_readable(cursor);
        let reader = Reader::new(source);

        let mut pager = Pager::new(reader, PageSize { rows: 3, cols: 8 });
        assert_eq!(
            page_to_vec(&mut pager.page())?,
            vec![
                ("abcde".to_string(), 1),
                ("1234567".to_string(), 1),
                ("foo".to_string(), 1),
            ]
        );

        let new_row_span = pager.scroll_down_one_row()?;
        assert_eq!(new_row_span, Some(RowSpan::new("bar", 1)));
        assert_eq!(
            page_to_vec(&mut pager.page())?,
            vec![
                ("1234567".to_string(), 1),
                ("foo".to_string(), 1),
                ("bar".to_string(), 1),
            ]
        );

        let new_row_span = pager.scroll_up_one_row()?;
        assert_eq!(new_row_span, Some(RowSpan::new("abcde", 1)));
        assert_eq!(
            page_to_vec(&mut pager.page())?,
            vec![
                ("abcde".to_string(), 1),
                ("1234567".to_string(), 1),
                ("foo".to_string(), 1),
            ]
        );

        pager.scroll_to_end()?;
        assert_eq!(
            page_to_vec(&mut pager.page())?,
            vec![
                // wrapped 2 lines == 3 rows
                ("bar".to_string(), 1),
                ("123456789".to_string(), 2),
            ]
        );

        Ok(())
    }

    #[test]
    fn display_lines_less_than_page() -> Result<(), AnyError> {
        let s = "abc\n123".to_string();
        let cursor = Cursor::new(s);
        let source = source::as_readable(cursor);
        let reader = Reader::new(source);

        let mut pager = Pager::new(reader, PageSize { rows: 5, cols: 5 });
        assert_eq!(
            page_to_vec(&mut pager.page())?,
            vec![("abc".to_string(), 1), ("123".to_string(), 1)],
        );

        pager.scroll_to_end()?;
        assert_eq!(
            page_to_vec(&mut pager.page())?,
            vec![("abc".to_string(), 1), ("123".to_string(), 1)],
        );

        Ok(())
    }
}

#[cfg(all(feature = "bench", test))]
mod bench {
    extern crate test;

    use std::fs::File;
    use test::Bencher;

    use crate::{
        error::AnyError,
        pager::{PageSize, Pager},
        reader::Reader,
        source,
    };

    #[bench]
    fn paging(b: &mut Bencher) -> Result<(), AnyError> {
        let file_path = "Cargo.lock";
        let file = File::open(file_path)?;
        let source = source::as_seekable(file);
        let reader = Reader::new(source);
        let mut pager = Pager::new(reader, PageSize::new(20, 80));
        b.iter(|| {
            let mut total = 0;
            let mut page = pager.page();
            while let Some(row_span) = page.next_row_span().unwrap() {
                total += row_span.line().len();
            }
            drop(page);

            while let Some(row_span) = pager.scroll_down_one_row().unwrap() {
                total += row_span.line().len();
            }
            pager.scroll_to_start().unwrap();
            let mut page = pager.page();
            while let Some(row_span) = page.next_row_span().unwrap() {
                total += row_span.line().len();
            }
            drop(page);

            pager.scroll_to_end().unwrap();
            let mut page = pager.page();
            while let Some(row_span) = page.next_row_span().unwrap() {
                total += row_span.line().len();
            }
            drop(page);

            total
        });
        Ok(())
    }
}
