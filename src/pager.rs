mod line;
mod page;

use std::ops::RangeBounds;

use regex::Regex;

use crate::{
    AppResult, SearchDirection,
    pager::{
        line::{LineSlice, PageLine},
        page::{EmptyPage, FilledPage, LineSliceIter},
    },
    reader::{LinePos, QueryLine, Reader},
    source::Source,
};

#[derive(Debug)]
pub(crate) struct PageSize {
    /// A number of columns of the page.
    cols: usize,
    /// A number of rows of the page.
    rows: usize,
}

impl PageSize {
    pub fn new(cols: usize, rows: usize) -> Self {
        Self { cols, rows }
    }

    #[inline]
    pub fn rows(&self) -> usize {
        self.rows
    }
}

#[derive(Debug)]
struct SearchState {
    query: Regex,
    direction: SearchDirection,
}

#[derive(Debug)]
enum Page {
    Filled(FilledPage),
    Empty(EmptyPage),
}

/// Pager clips text lines to fit them in the sized frame. The frame is called a page.
/// Its responsibilites are:
/// 1. Read source text and load lines to a page.
/// 2. Determine currently "visible" rows. This role is internally delegated to [`FilledPage`].
/// 3. Wrap lines based on the column size.
#[derive(Debug)]
pub(crate) struct Pager<R, Src> {
    reader: Reader<R, Src>,
    page: Page,
    line_factory: PageLineFactory,
}

impl<R, Src: Source<R>> Pager<R, Src> {
    pub fn new(mut reader: Reader<R, Src>, size: PageSize) -> AppResult<Self> {
        let page = match build_page(&mut reader, &size)? {
            None => Page::Empty(EmptyPage::new()),
            Some(page) => Page::Filled(page),
        };
        Ok(Self {
            reader,
            page,
            line_factory: PageLineFactory {
                page_size: size,
                search: None,
            },
        })
    }

    #[inline]
    pub fn size(&self) -> &PageSize {
        &self.line_factory.page_size
    }

    pub fn line_slices(&mut self, row_range: impl RangeBounds<usize>) -> LineSliceIter<'_> {
        match &self.page {
            Page::Filled(page) => page.line_slices(row_range),
            Page::Empty(page) => page.line_slices(),
        }
    }

    pub fn scroll_down_one_row(&mut self) -> AppResult<Option<LineSlice<'_>>> {
        let page = match &mut self.page {
            Page::Empty(_) => return Ok(None),
            Page::Filled(page) => page,
        };
        if !page.move_down_one_row() {
            let end_pos = *page.end_line().pos();
            match self.reader.read_line(&QueryLine::NextOf(end_pos))? {
                None => {
                    log::debug!("Pager: not scrolled down: {:?}", page);
                    return Ok(None);
                }
                Some((pos, text)) => {
                    let line = self.line_factory.new_line(pos, text);
                    page.move_down_one_line(line);
                }
            }
        }
        log::debug!("Pager: scrolled down: {:?}", page);
        Ok(Some(page.end_line_slice()))
    }

    pub fn scroll_up_one_row(&mut self) -> AppResult<Option<LineSlice<'_>>> {
        let page = match &mut self.page {
            Page::Empty(_) => return Ok(None),
            Page::Filled(page) => page,
        };
        if !page.move_up_one_row() {
            let start_pos = *page.start_line().pos();
            match self.reader.read_line(&QueryLine::PrevOf(start_pos))? {
                None => {
                    log::debug!("Pager: not scrolled up: {:?}", page);
                    return Ok(None);
                }
                Some((pos, text)) => {
                    let line = self.line_factory.new_line(pos, text);
                    page.move_up_one_line(line);
                }
            }
        }
        log::debug!("Pager: scrolled up: {:?}", page);
        Ok(Some(page.start_line_slice()))
    }

    pub fn scroll_to_start(&mut self) -> AppResult<bool> {
        let page = match &mut self.page {
            Page::Empty(_) => return Ok(false),
            Page::Filled(page) => page,
        };
        if page.start_line().pos().is_first_line() && !page.can_move_up_one_row() {
            return Ok(false);
        }
        write_page_from(
            QueryLine::AtStart,
            &mut self.reader,
            page,
            &self.line_factory,
        )?;
        Ok(true)
    }

    pub fn scroll_to_end(&mut self) -> AppResult<bool> {
        let page = match &mut self.page {
            Page::Empty(_) => return Ok(false),
            Page::Filled(page) => page,
        };
        let end_pos_before = *page.end_line().pos();
        let end_line_continue_before = page.can_move_down_one_row();
        write_page_ending_at(QueryLine::AtEnd, &mut self.reader, page, &self.line_factory)?;
        Ok(end_line_continue_before || page.end_line().pos() != &end_pos_before)
    }

    pub fn search(&mut self, query: Regex, direction: SearchDirection) -> AppResult<bool> {
        if matches!(direction, SearchDirection::Down) {
            self.search_forward(query)
        } else {
            self.search_backward(query)
        }
    }

    fn search_forward(&mut self, query: Regex) -> AppResult<bool> {
        let page = match &mut self.page {
            Page::Empty(_) => return Ok(false),
            Page::Filled(page) => page,
        };
        let line_from = match page.find_first_match_line(&query) {
            Some(matching) => QueryLine::At(*matching.pos()),
            None => {
                let line_from = QueryLine::NextOf(*page.end_line().pos());
                match self.reader.find_first_match_line(line_from, &query)? {
                    None => return Ok(false),
                    Some((pos, _)) => QueryLine::At(pos),
                }
            }
        };
        self.line_factory.search = Some(SearchState {
            query,
            direction: SearchDirection::Down,
        });
        write_page_from(line_from, &mut self.reader, page, &self.line_factory)?;
        Ok(true)
    }

    fn search_backward(&mut self, query: Regex) -> AppResult<bool> {
        let page = match &mut self.page {
            Page::Empty(_) => return Ok(false),
            Page::Filled(page) => page,
        };
        let line_from = match page.find_last_match_line(&query) {
            Some(line) => QueryLine::At(*line.pos()),
            None => {
                let line_from = QueryLine::PrevOf(*page.start_line().pos());
                match self.reader.find_last_match_line_before(line_from, &query)? {
                    None => return Ok(false),
                    Some((pos, _)) => QueryLine::At(pos),
                }
            }
        };
        self.line_factory.search = Some(SearchState {
            query,
            direction: SearchDirection::Up,
        });
        write_page_from(line_from, &mut self.reader, page, &self.line_factory)?;
        Ok(true)
    }

    /// Jump to the next search match and return the number of rows to be scrolled.
    pub fn jump_to_next_search_match(&mut self) -> AppResult<usize> {
        self.jump_to_next_search_match_with(false)
    }

    /// Jump to the next search match in reversed order and return the number of rows to be scrolled.
    pub fn jump_to_next_search_match_reversed(&mut self) -> AppResult<usize> {
        self.jump_to_next_search_match_with(true)
    }

    fn jump_to_next_search_match_with(&mut self, reversed: bool) -> AppResult<usize> {
        let page = match &mut self.page {
            Page::Empty(_) => return Ok(0),
            Page::Filled(page) => page,
        };
        let search = match &self.line_factory.search {
            None => return Ok(0),
            Some(search) => search,
        };
        let go_down = if reversed {
            matches!(search.direction, SearchDirection::Up)
        } else {
            matches!(search.direction, SearchDirection::Down)
        };
        // Find the next or previous match from the current one.
        // The current one is always the first line (+ `--jump-target`) of the page.
        if go_down {
            match page.find_second_match_line(&search.query) {
                Some(matching) => {
                    let line_from = QueryLine::At(*matching.pos());
                    write_page_from(line_from, &mut self.reader, page, &self.line_factory)?;
                    Ok(matching.passed_rows())
                }
                None => {
                    let line_from = QueryLine::NextOf(*page.end_line().pos());
                    match self
                        .reader
                        .find_first_match_line(line_from, &search.query)?
                    {
                        None => Ok(0),
                        Some((pos, _)) => {
                            let line_from = QueryLine::At(pos);
                            write_page_from(line_from, &mut self.reader, page, &self.line_factory)?;
                            Ok(page.row_len())
                        }
                    }
                }
            }
        } else {
            match self.reader.find_last_match_line_before(
                QueryLine::PrevOf(*page.start_line().pos()),
                &search.query,
            )? {
                None => Ok(0),
                Some((pos, _)) => {
                    let line_from = QueryLine::At(pos);
                    write_page_from(line_from, &mut self.reader, page, &self.line_factory)?;
                    Ok(page.row_len())
                }
            }
        }
    }
}

fn build_page<R, Src: Source<R>>(
    reader: &mut Reader<R, Src>,
    size: &PageSize,
) -> AppResult<Option<FilledPage>> {
    let mut builder = FilledPage::builder(size.rows);
    let mut lines = reader.lines_from(QueryLine::AtStart);
    while let Some((pos, text)) = lines.next()? {
        let line = PageLine::new(pos, text, size.cols);
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
    page: &mut FilledPage,
    line_factory: &PageLineFactory,
) -> AppResult<()> {
    let mut writer = page.forward_page_writer();
    let mut lines = reader.lines_from(query);
    while let Some((pos, text)) = lines.next()? {
        let line = line_factory.new_line(pos, text);
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
    page: &mut FilledPage,
    line_factory: &PageLineFactory,
) -> AppResult<()> {
    let mut writer = page.backward_page_writer();
    let mut lines = reader.lines_rev_from(query);
    while let Some((pos, text)) = lines.next()? {
        let line = line_factory.new_line(pos, text);
        if !writer.push_front(line) {
            break;
        }
    }
    writer.write_to_page();
    Ok(())
}

#[derive(Debug)]
struct PageLineFactory {
    page_size: PageSize,
    search: Option<SearchState>,
}

impl PageLineFactory {
    fn new_line(&self, pos: LinePos, text: String) -> PageLine {
        let mut line = PageLine::new(pos, text, self.page_size.cols);
        if let Some(search) = &self.search {
            line.search(&search.query);
        }
        line
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use pretty_assertions::assert_eq;

    use crate::{
        AppResult,
        pager::{PageSize, Pager},
        reader::Reader,
        source,
    };

    #[test]
    fn pager_scroll_up_down() -> AppResult<()> {
        let s = "abcde\n1234567\nfoo\nbar\n123456789".to_string();
        let cursor = Cursor::new(s);
        let source = source::as_readable(cursor);
        let reader = Reader::new(source);

        let mut pager = Pager::new(reader, PageSize { rows: 3, cols: 8 })?;
        assert_eq!(
            pager.line_slices(..).into_vec(),
            vec![
                ("abcde".to_string(), 1),
                ("1234567".to_string(), 1),
                ("foo".to_string(), 1),
            ]
        );

        pager.scroll_down_one_row()?;
        assert_eq!(
            pager.line_slices(..).into_vec(),
            vec![
                ("1234567".to_string(), 1),
                ("foo".to_string(), 1),
                ("bar".to_string(), 1),
            ]
        );

        pager.scroll_to_end()?;
        assert_eq!(
            pager.line_slices(..).into_vec(),
            vec![
                // Only two row spans since the total rows are 3 (max).
                ("bar".to_string(), 1),
                ("123456789".to_string(), 2),
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
        AppResult,
        pager::{PageSize, Pager},
        reader::Reader,
        source,
    };

    #[bench]
    fn paging(b: &mut Bencher) -> AppResult<()> {
        let file_path = "Cargo.lock";
        let file = File::open(file_path)?;
        let source = source::as_seekable(file);
        let reader = Reader::new(source);
        let mut pager = Pager::new(reader, PageSize::new(80, 20))?;
        b.iter(|| {
            let mut total = 0;
            while let Some(line_slice) = pager.scroll_down_one_row().unwrap() {
                total += line_slice.to_string().len();
            }
            pager.scroll_to_start().unwrap();
            pager.scroll_to_end().unwrap();
            total
        });
        Ok(())
    }
}
