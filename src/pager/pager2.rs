use std::fs::read;

use crate::{
    error::AnyError,
    pager::{
        PageSize,
        line::RowSpan,
        line_reader::{LineReader, PageLine},
    },
    reader::{LinePos, QueryLine, Reader},
    source::Source,
};

#[derive(Debug, PartialEq)]
struct Row {
    line_pos: LinePos,
    slice_index: usize,
    end_slice_index: usize,
}

impl Row {
    fn from_line(line: &PageLine, slice_index: usize) -> Self {
        Self {
            line_pos: *line.meta(),
            slice_index,
            end_slice_index: line.row_len() - 1,
        }
    }
}

#[derive(Debug)]
pub(crate) struct Pager<R, Src> {
    line_reader: LineReader<R, Src>,
    size: PageSize,
    start_row: Option<Row>,
    end_row: Option<Row>,
}

impl<R, Src: Source<R>> Pager<R, Src> {
    pub fn new(reader: Reader<R, Src>, size: PageSize) -> Self {
        Self {
            line_reader: LineReader::new(reader, &size),
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
        let end_row = match move_down_row(&mut self.line_reader, self.end_row.take())? {
            None => return Ok(None),
            Some(row) => row,
        };
        if self.start_row != self.end_row {
            self.start_row = move_down_row(&mut self.line_reader, self.start_row.take())?;
        }
        let query = QueryLine::At(end_row.line_pos);
        let row_span = match self.line_reader.read_line(query)? {
            None => None,
            Some(line) => {
                let row_span = line.slice(..=end_row.slice_index);
                Some(row_span)
            }
        };
        self.end_row = Some(end_row);
        Ok(row_span)
    }

    pub fn scroll_up_one_row(&mut self) -> Result<Option<RowSpan<'_>>, AnyError> {
        let start_row = match move_up_row(&mut self.line_reader, self.start_row.take())? {
            None => return Ok(None),
            Some(row) => row,
        };
        if self.start_row != self.end_row {
            self.end_row = move_up_row(&mut self.line_reader, self.end_row.take())?;
        }
        let query = QueryLine::At(start_row.line_pos);
        let row_span = match self.line_reader.read_line(query)? {
            None => None,
            Some(line) => {
                let row_span = line.slice(start_row.slice_index..);
                Some(row_span)
            }
        };
        self.start_row = Some(start_row);
        Ok(row_span)
    }

    pub fn scroll_to_start(&mut self) -> Result<(), AnyError> {
        self.start_row = None;
        self.end_row = None;
        Ok(())
    }

    // XXX: これやって page() でいけるかと思ったが、 query 基準だと必ず
    // start_row の先頭から始めちゃう。 start_row が line の中間のケースで正しく動かない。
    // そもそも page() のように読むだけなら start/end row を更新する必要がない問題もある。
    pub fn scroll_to_end(&mut self) -> Result<(), AnyError> {
        PageLoader::backward(self, QueryLine::AtEnd).run()
    }

    // 現状を見るだけなら本来は start/end_row の初期化は不要なんだが、
    // そこを変えないバージョンを作るにはコピペしか思いつかず、面倒。
    pub fn page(&mut self) -> PageLoader<'_, R, Src> {
        PageLoader::forward_from_current(self)
    }
}

fn move_down_row<'r, R, Src: Source<R>>(
    line_reader: &'r mut LineReader<R, Src>,
    row: Option<Row>,
) -> Result<Option<Row>, AnyError> {
    let mut row = match row {
        None => return Ok(None),
        Some(row) => row,
    };
    if row.slice_index < row.end_slice_index {
        row.slice_index += 1;
        return Ok(Some(row));
    }
    let query = QueryLine::NextOf(row.line_pos);
    match line_reader.read_line(query)? {
        None => Ok(Some(row)),
        Some(line) => Ok(Some(Row::from_line(&line, 0))),
    }
}

fn move_up_row<'r, R, Src: Source<R>>(
    line_reader: &'r mut LineReader<R, Src>,
    row: Option<Row>,
) -> Result<Option<Row>, AnyError> {
    let mut row = match row {
        None => return Ok(None),
        Some(row) => row,
    };
    if 0 < row.slice_index {
        row.slice_index -= 1;
        return Ok(Some(row));
    }
    let query = QueryLine::PrevOf(row.line_pos);
    match line_reader.read_line(query)? {
        None => return Ok(Some(row)),
        Some(line) => Ok(Some(Row::from_line(&line, line.row_len() - 1))),
    }
}

pub(crate) struct PageLoader<'p, R, Src> {
    pager: &'p mut Pager<R, Src>,
    read_rows: usize,
    query: QueryLine,
    first_line_slice_index: Option<usize>,
    go_forward: bool,
}

impl<'p, R, Src: Source<R>> PageLoader<'p, R, Src> {
    fn forward(pager: &'p mut Pager<R, Src>, from: QueryLine) -> Self {
        pager.start_row = None;
        pager.end_row = None;
        Self {
            pager,
            read_rows: 0,
            query: from,
            first_line_slice_index: None,
            go_forward: true,
        }
    }

    fn forward_from_current(pager: &'p mut Pager<R, Src>) -> Self {
        let (query, first_line_slice_index) = match &pager.start_row {
            None => (QueryLine::AtStart, None),
            Some(row) => (QueryLine::At(row.line_pos), Some(row.slice_index)),
        };
        pager.start_row = None;
        pager.end_row = None;
        Self {
            pager,
            read_rows: 0,
            query,
            first_line_slice_index,
            go_forward: true,
        }
    }

    fn backward(pager: &'p mut Pager<R, Src>, from: QueryLine) -> Self {
        pager.start_row = None;
        pager.end_row = None;
        Self {
            pager,
            read_rows: 0,
            query: from,
            first_line_slice_index: None,
            go_forward: false,
        }
    }

    // fn new(pager: &'p mut Pager<R, Src>, from: QueryLine, go_forward: bool) -> Self {
    //     pager.start_row = None;
    //     pager.end_row = None;
    //     Self {
    //         pager,
    //         read_rows: 0,
    //         query: from,
    //         go_forward,
    //     }
    // }

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
        match self.pager.line_reader.read_line(self.query)? {
            None => Ok(None),
            Some(line) => {
                let start_slice_idx = self.first_line_slice_index.take().unwrap_or(0);
                if self.pager.start_row.is_none() {
                    self.pager.start_row = Some(Row::from_line(&line, start_slice_idx));
                }
                self.read_rows += line.row_len() - start_slice_idx;
                let (end_slice_idx, line) = if self.read_rows <= row_size {
                    (line.row_len() - 1, line)
                } else {
                    let end_slice_idx = line.row_len() - 1 - (self.read_rows - row_size);
                    // dbg!((end_slice_idx, line.slice(..)));
                    (end_slice_idx, line)
                };
                self.pager.end_row = Some(Row::from_line(&line, end_slice_idx));
                self.query = QueryLine::NextOf(*line.meta());
                Ok(Some(line.slice(start_slice_idx..=end_slice_idx)))
            }
        }
    }

    // XXX: first_line_slice_index
    fn next_backward(&mut self) -> Result<Option<RowSpan<'_>>, AnyError> {
        let row_size = self.pager.size().rows();
        if self.read_rows >= row_size {
            return Ok(None);
        }
        match self.pager.line_reader.read_line(self.query)? {
            None => Ok(None),
            Some(line) => {
                if self.pager.end_row.is_none() {
                    self.pager.end_row = Some(Row::from_line(&line, line.row_len() - 1));
                }
                self.read_rows += line.row_len();
                let (start_slice_idx, line) = if self.read_rows <= row_size {
                    (0, line)
                } else {
                    let start_slice_idx = self.read_rows - row_size;
                    (start_slice_idx, line)
                };
                self.pager.start_row = Some(Row::from_line(&line, start_slice_idx));
                self.query = QueryLine::PrevOf(*line.meta());
                Ok(Some(line.slice(start_slice_idx..)))
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
        pager::{
            PageSize,
            line::RowSpan,
            pager2::{PageLoader, Pager},
        },
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
}
