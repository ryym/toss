use crate::{
    error::AnyError,
    pager::{
        line::{PageLine, RowSpan},
        page::{Page, RowSpanIter},
    },
    reader::{LinePos, QueryLine, Reader},
    source::Source,
};

mod line;
mod line_reader;
mod page;
mod pager2;

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

#[derive(Debug)]
pub(crate) struct LineMeta {
    pos: LinePos,
}

// #[derive(Debug)]
// pub(crate) struct Query {
//     search_id: usize,
//     // query: regex,
// }

/// Pager clips text lines to fit them in the fixed size frame. The frame is called a page.
/// Its responsibilites are:
/// 1. Read source text and load to a page.
/// 2. Determine lines that are currently "visible" in the page.
/// 3. Wrap lines based on the column size.
#[derive(Debug)]
pub(crate) struct Pager<R, Src> {
    reader: Reader<R, Src>,
    size: PageSize,
    page: Page<LineMeta>,
}

impl<R, Src: Source<R>> Pager<R, Src> {
    pub fn new(mut reader: Reader<R, Src>, size: PageSize) -> Result<Option<Self>, AnyError> {
        match build_page(&mut reader, &size)? {
            None => Ok(None),
            Some(page) => Ok(Some(Self { reader, size, page })),
        }
    }

    #[inline]
    pub fn size(&self) -> &PageSize {
        &self.size
    }

    pub fn row_spans(&mut self) -> RowSpanIter<'_, LineMeta> {
        self.page.row_spans()
    }

    pub fn scroll_down_one_row(&mut self) -> Result<Option<RowSpan<'_>>, AnyError> {
        if !self.page.move_down_one_row() {
            let end_pos = self.page.end_line().meta().pos;
            match self.reader.read_line(QueryLine::NextOf(end_pos))? {
                None => return Ok(None),
                Some((pos, text)) => {
                    let line = PageLine::new(LineMeta { pos }, text, self.size.cols);
                    self.page.push_back(line);
                    self.page.move_down_one_row();
                }
            }
        }
        Ok(Some(self.page.end_row_span()))
    }

    pub fn scroll_up_one_row(&mut self) -> Result<Option<RowSpan<'_>>, AnyError> {
        if !self.page.move_up_one_row() {
            let start_pos = self.page.start_line().meta().pos;
            match self.reader.read_line(QueryLine::PrevOf(start_pos))? {
                None => return Ok(None),
                Some((pos, text)) => {
                    let line = PageLine::new(LineMeta { pos }, text, self.size.cols);
                    self.page.push_front(line);
                    self.page.move_up_one_row();
                }
            }
        }
        Ok(Some(self.page.start_row_span()))
    }

    pub fn scroll_to_start(&mut self) -> Result<(), AnyError> {
        write_start_page(&mut self.reader, &self.size, &mut self.page)?;
        Ok(())
    }

    pub fn scroll_to_end(&mut self) -> Result<(), AnyError> {
        write_end_page(&mut self.reader, &self.size, &mut self.page)?;
        Ok(())
    }

    // pub fn search(&mut self, query: &Query) -> Result<(), AnyError> {
    //     // let pos = match self.page.find_first_match(&query) {
    //     //     Some(line) => line.meta().pos,
    //     //     None => self.reader.find_first_match(&query)?,
    //     // };

    //     // XXX: ここで行を描画しようとした時、 page と reader が別れてるとやりづらいな。
    //     // page にある部分までは page から (?) で、他は reader からみたいな。
    //     // やはり reader へのアクセスを抽象化してくれるやつがいる方が、汎用的だし使いやすいか？

    //     // 仮に line reader 相当のやつを作ったらどうなる？
    //     // Pager は start, end row を管理して、 App 側でのループは？
    //     // line (row span) cursor っぽいのを app 側で使う形に？
    //     // highlight 処理は pager？ line_reader はキャッシュに専念してほしいが。
    //     // app 側がループする際に highlight する感じかなぁ。
    //     // &mut なアイテムを iterator-like に返せるっけ...？

    //     let pos = self.line_reader.find_first_match(&query)?;
    //     let lines = self.line_reader.lines_from(pos);
    //     // while let Some(line) = lines.next() {
    //     //     // highlight??
    //     // }

    //     // 検索後の n/N はどうしよう。
    //     // 別途 next_match() がたぶんいて、enum を返しそう。
    //     // InPage { n_scrolls, row_spans }
    //     // Jump { row_spans }

    //     todo!()
    // }
}

fn build_page<R, Src: Source<R>>(
    reader: &mut Reader<R, Src>,
    size: &PageSize,
) -> Result<Option<Page<LineMeta>>, AnyError> {
    let mut builder = Page::builder(size.rows);
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

fn write_start_page<R, Src: Source<R>>(
    reader: &mut Reader<R, Src>,
    size: &PageSize,
    page: &mut Page<LineMeta>,
) -> Result<(), AnyError> {
    let mut writer = page.start_page_writer();
    let mut lines = reader.lines_from(QueryLine::AtStart);
    while let Some((pos, text)) = lines.next()? {
        let line = PageLine::new(LineMeta { pos }, text, size.cols);
        if !writer.push_back(line) {
            break;
        }
    }
    writer.write_to_page();
    Ok(())
}

fn write_end_page<R, Src: Source<R>>(
    reader: &mut Reader<R, Src>,
    size: &PageSize,
    page: &mut Page<LineMeta>,
) -> Result<(), AnyError> {
    let mut writer = page.end_page_writer();
    let mut lines = reader.lines_rev_from(QueryLine::AtEnd);
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

        let mut pager = Pager::new(reader, PageSize { rows: 3, cols: 8 })?.expect("build pager");
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
