use crate::{
    error::AnyError,
    pager::{
        line::{PageLine, RowSpan},
        page::{EmptyPage, FilledPage, RowSpanIter},
    },
    reader::{LinePos, QueryLine, Reader},
    source::Source,
};

mod line;
mod page;

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
}

#[derive(Debug)]
pub(crate) struct LineMeta {
    pos: LinePos,
}

#[derive(Debug)]
enum Page {
    Filled(FilledPage<LineMeta>),
    Empty(EmptyPage<LineMeta>),
}

/// Pager clips text lines to fit them in the sized frame. The frame is called a page.
/// Its responsibilites are:
/// 1. Read source text and load to a page.
/// 2. Determine lines that are currently "visible" in the page.
/// 3. Wrap lines based on the column size.
#[derive(Debug)]
pub(crate) struct Pager<R, Src> {
    reader: Reader<R, Src>,
    size: PageSize,
    page: Page,
}

impl<R, Src: Source<R>> Pager<R, Src> {
    pub fn new(mut reader: Reader<R, Src>, size: PageSize) -> Result<Self, AnyError> {
        let page = match build_page(&mut reader, &size)? {
            None => Page::Empty(EmptyPage::new()),
            Some(page) => Page::Filled(page),
        };
        Ok(Self { reader, size, page })
    }

    #[inline]
    pub fn size(&self) -> &PageSize {
        &self.size
    }

    pub fn row_spans(&mut self) -> RowSpanIter<'_, LineMeta> {
        match &self.page {
            Page::Filled(page) => page.row_spans(),
            Page::Empty(page) => page.row_spans(),
        }
    }

    pub fn scroll_down_one_row(&mut self) -> Result<Option<RowSpan<'_>>, AnyError> {
        let page = match &mut self.page {
            Page::Empty(_) => return Ok(None),
            Page::Filled(page) => page,
        };
        if !page.move_down_one_row() {
            let end_pos = page.end_line().meta().pos;
            match self.reader.read_line(&QueryLine::NextOf(end_pos))? {
                None => return Ok(None),
                Some((pos, text)) => {
                    let line = PageLine::new(LineMeta { pos }, text, self.size.cols);
                    page.move_down_one_line(line);
                }
            }
        }
        Ok(Some(page.end_row_span()))
    }

    pub fn scroll_up_one_row(&mut self) -> Result<Option<RowSpan<'_>>, AnyError> {
        let page = match &mut self.page {
            Page::Empty(_) => return Ok(None),
            Page::Filled(page) => page,
        };
        if !page.move_up_one_row() {
            let start_pos = page.start_line().meta().pos;
            match self.reader.read_line(&QueryLine::PrevOf(start_pos))? {
                None => return Ok(None),
                Some((pos, text)) => {
                    let line = PageLine::new(LineMeta { pos }, text, self.size.cols);
                    page.move_up_one_line(line);
                }
            }
        }
        Ok(Some(page.start_row_span()))
    }

    pub fn scroll_to_start(&mut self) -> Result<bool, AnyError> {
        let page = match &mut self.page {
            Page::Empty(_) => return Ok(false),
            Page::Filled(page) => page,
        };
        if page.start_line().meta().pos.is_first_line() {
            return Ok(false);
        }
        write_page_from(QueryLine::AtStart, &mut self.reader, &self.size, page)?;
        Ok(true)
    }

    pub fn scroll_to_end(&mut self) -> Result<bool, AnyError> {
        let page = match &mut self.page {
            Page::Empty(_) => return Ok(false),
            Page::Filled(page) => page,
        };
        let end_pos_before = page.end_line().meta().pos;
        write_page_ending_at(QueryLine::AtEnd, &mut self.reader, &self.size, page)?;
        Ok(page.end_line().meta().pos != end_pos_before)
    }
}

fn build_page<R, Src: Source<R>>(
    reader: &mut Reader<R, Src>,
    size: &PageSize,
) -> Result<Option<FilledPage<LineMeta>>, AnyError> {
    let mut builder = FilledPage::builder(size.rows);
    let mut lines = reader.lines_from(QueryLine::AtStart);
    while let Some((pos, text)) = lines.next()? {
        let line = PageLine::new(LineMeta { pos }, text, size.cols);
        if !builder.push_back(line) {
            break;
        }
    }
    match builder.into_page() {
        Some(page) => Ok(Some(page)),
        _ => Ok(None),
    }
}

fn write_page_from<R, Src: Source<R>>(
    query: QueryLine,
    reader: &mut Reader<R, Src>,
    size: &PageSize,
    page: &mut FilledPage<LineMeta>,
) -> Result<(), AnyError> {
    let mut writer = page.forward_page_writer();
    let mut lines = reader.lines_from(query);
    while let Some((pos, text)) = lines.next()? {
        let line = PageLine::new(LineMeta { pos }, text, size.cols);
        if !writer.push_back(line) {
            break;
        }
    }
    writer.write_to_page();
    Ok(())
}

fn write_page_ending_at<R, Src: Source<R>>(
    query: QueryLine,
    reader: &mut Reader<R, Src>,
    size: &PageSize,
    page: &mut FilledPage<LineMeta>,
) -> Result<(), AnyError> {
    let mut writer = page.backward_page_writer();
    let mut lines = reader.lines_rev_from(query);
    while let Some((pos, text)) = lines.next()? {
        let line = PageLine::new(LineMeta { pos }, text, size.cols);
        if !writer.push_front(line) {
            break;
        }
    }
    writer.write_to_page();
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use pretty_assertions::assert_eq;

    use crate::{
        error::AnyError,
        pager::{PageSize, Pager, line::RowSpan},
        reader::Reader,
        source,
    };

    #[test]
    fn pager_scroll_up_down() -> Result<(), AnyError> {
        let s = "abcde\n1234567\nfoo\nbar\n123456789".to_string();
        let cursor = Cursor::new(s);
        let source = source::as_readable(cursor);
        let reader = Reader::new(source);

        let mut pager = Pager::new(reader, PageSize { rows: 3, cols: 8 })?;
        assert_eq!(
            pager.row_spans().collect::<Vec<_>>(),
            vec![
                RowSpan::new("abcde", 1),
                RowSpan::new("1234567", 1),
                RowSpan::new("foo", 1),
            ]
        );

        pager.scroll_down_one_row()?;
        assert_eq!(
            pager.row_spans().collect::<Vec<_>>(),
            vec![
                RowSpan::new("1234567", 1),
                RowSpan::new("foo", 1),
                RowSpan::new("bar", 1),
            ]
        );

        pager.scroll_to_end()?;
        assert_eq!(
            pager.row_spans().collect::<Vec<_>>(),
            vec![
                // Only two row spans since the total rows are 3 (max).
                RowSpan::new("bar", 1),
                RowSpan::new("123456789", 2),
            ]
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
        let mut pager = Pager::new(reader, PageSize::new(20, 80))?;
        b.iter(|| {
            let mut total = 0;
            while let Some(row_span) = pager.scroll_down_one_row().unwrap() {
                total += row_span.line().len();
            }
            pager.scroll_to_start().unwrap();
            pager.scroll_to_end().unwrap();
            total
        });
        Ok(())
    }
}
