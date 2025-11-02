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
mod line_deque;
mod page;

// シンプルに全くキャッシュを持たない形で始められないかと思ったが、
// その場合は例えば初期表示時にページ分の行を読みつつ、即 app 側が各行にアクセスできないといけない。
// iterator っぽい形にはできると思うが、呼び出し側が途中でやめたら Pager 内は中途半端な
// 状態になっちゃう (end_row が正しくセットされない)。別にライブラリ内でしか使わないんだから
// 呼び出し側を信用しても全然いいんだが、内部状態が変になりえない作りの方がもちろん良い。
// かといって事前に end_row まで算出した上で、呼び出し側に改めて iterate させるなら、
// キャッシュが全くないのはさすがに変に感じる。
// それなら単純な LRU の行キャッシュを挟んどけば汎用的だし良さそうと思ったが、
// 逆方向に行をたどる時、次に読み出したい行の先頭位置が不明なので、「まずキャッシュを見てなければ read」
// ができない (先頭位置をキーにしたら)。終端->先頭のマップを別途持つとか、LinePos をキーにして
// keys を走査して探すとか、考えられる方法はあるにはあるが、結局あまりシンプルではなくなる。
// と考えていくと、結局 VecDeque を使うのが一番単純かもしれない。
// キャッシュの量はキャパシティで調整できるし、上下どちらへ移動する場合でも、
// すでに行がロードされてるかを確認してなければ read 、というのが原理的にはできるはず。
// 行末とかにジャンプされるたびにキャッシュは無意味になるが、上下の近い行をキャッシュできるだけでも意義はありそう。
// この方針だと画面に表示される行は最低限必ず Deque にある必要があり、巨大なモニターとかだと量が増えるが、
// まあそれが問題になることはあまりなさそう。

// 検索機能
// 1. App: Pager に検索文字列を設定する。
// 2. Pager: Page 内のキャッシュをまず検索する。
//   a. Pager: Page 内のキャッシュになければ、検索済み位置の次以降の Source 内を検索する。
// 3. 検索にヒットした最初の行を先頭に、新しいページを描画する。
//   vim みたいに、見えてるとこにある場合は移動しないのでもいいけど。
// considerations:
// - 検索は行ごとに行う。同じ正規表現のインスタンスを使い回す必要がある。
// - 検索ヒットの有無のみならず、ヒット位置をハイライトした文字列がヒット行ごとに必要。

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
pub(crate) struct Pager<R, Src> {
    reader: Reader<R, Src>,
    size: PageSize,
    page: Page<LineMeta>,
    // start_pos: LinePos,
    // end_pos: LinePos,
}

impl<R, Src: Source<R>> Pager<R, Src> {
    pub fn new(mut reader: Reader<R, Src>, size: PageSize) -> Result<Option<Self>, AnyError> {
        match build_page(&mut reader, &size)? {
            None => Ok(None),
            Some(page) => Ok(Some(Self {
                reader,
                size,
                page,
                // start_pos,
                // end_pos,
            })),
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
}

fn build_page<R, Src: Source<R>>(
    reader: &mut Reader<R, Src>,
    size: &PageSize,
) -> Result<Option<Page<LineMeta>>, AnyError> {
    let mut builder = Page::builder(size.rows);
    let mut query = QueryLine::AtStart;
    loop {
        match reader.read_line(query)? {
            None => break,
            Some((pos, text)) => {
                let line = PageLine::new(LineMeta { pos }, text, size.cols);
                if !builder.push_back(line) {
                    break;
                }
                query = QueryLine::NextOf(pos);
            }
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
    let mut query = QueryLine::AtStart;
    loop {
        match reader.read_line(query)? {
            None => break,
            Some((pos, text)) => {
                let line = PageLine::new(LineMeta { pos }, text, size.cols);
                if !writer.push_back(line) {
                    break;
                }
                query = QueryLine::NextOf(pos);
            }
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
    let mut query = QueryLine::AtEnd;
    loop {
        match reader.read_line(query)? {
            None => break,
            Some((pos, text)) => {
                let line = PageLine::new(LineMeta { pos }, text, size.cols);
                if !writer.push_front(line) {
                    break;
                }
                query = QueryLine::PrevOf(pos);
            }
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
        pager::{line::RowSpan, PageSize, Pager},
        reader::Reader,
        source,
    };

    #[test]
    fn hoge() -> Result<(), AnyError> {
        let s = "abcde\n1234567\nfoo\nbar\nbaz".to_string();
        let cursor = Cursor::new(s);
        let source = source::as_readable(cursor);
        let reader = Reader::new(source);
        let mut pager = Pager::new(reader, PageSize { rows: 3, cols: 10 })?.expect("build pager");

        assert_eq!(
            pager.row_spans().collect::<Vec<_>>(),
            vec![
                RowSpan::new("abcde", 1),
                RowSpan::new("1234567", 1),
                RowSpan::new("foo", 1),
            ]
        );

        // let new_row_span = pager.scroll_down_one_row()?;

        Ok(())
    }
}
