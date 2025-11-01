use std::collections::VecDeque;

use crate::pager::line::{PageLine, RowSpan};

#[derive(Debug)]
struct Row {
    deque_index: usize,
    slice_index: usize,
}

impl PartialEq for Row {
    fn eq(&self, other: &Self) -> bool {
        self.deque_index == other.deque_index && self.slice_index == other.slice_index
    }
}

pub(super) struct PageBuilder<LineMeta> {
    deque: VecDeque<PageLine<LineMeta>>,
    end_row: Option<Row>,
    row_size: usize,
    read_rows: usize,
}

impl<LineMeta> PageBuilder<LineMeta> {
    fn new(row_size: usize) -> Self {
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

        self.read_rows += line.row_len();
        if self.read_rows < self.row_size {
            self.deque.push_back(line);
            return true;
        }

        let end_slice_idx = line.row_len() - 1 - (self.read_rows - self.row_size);
        self.end_row = Some(Row {
            deque_index: self.deque.len(),
            slice_index: end_slice_idx,
        });
        self.deque.push_back(line);
        false
    }

    pub fn to_page(self) -> Option<Page<LineMeta>> {
        if self.deque.is_empty() {
            return None;
        }
        let start_row = Row {
            deque_index: 0,
            slice_index: 0,
        };
        // The end row is not set when lines are less than the page size.
        let end_row = self.end_row.unwrap_or_else(|| Row {
            deque_index: self.deque.len() - 1,
            slice_index: self.deque[self.deque.len() - 1].row_len() - 1,
        });
        Some(Page {
            deque: self.deque,
            row_size: self.row_size,
            start_row,
            end_row,
        })
    }
}

#[derive(Debug)]
pub(super) struct Page<LineMeta> {
    deque: VecDeque<PageLine<LineMeta>>,
    row_size: usize,
    start_row: Row,
    end_row: Row,
}

impl<LineMeta> Page<LineMeta> {
    pub fn builder(row_size: usize) -> PageBuilder<LineMeta> {
        PageBuilder::new(row_size)
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
        // 表示 line 数が capacity よりも少ない (wrapped line がある) 場合
        //   start_row が 0 行目だったら
        //     pop は絶対に不要。まだ capacity に余裕がある。
        //   start_row.line > 0 だったら

        // variables
        //   original lines
        //   capacity
        //   display size
        //   (wrapped or not)
        // invariant
        //   display size < capacity
        //
        // ->
        //   if original lines <= capacity
        //     -> never pop
        //     wrap の有無は関係 なし。 wrap があったら display size 内の lines が減るだけ
        //  if original lines > capacity
        //    wrap のことを考えなければ、 pop が発生するのは必ず display の外。なぜなら display size < capacity
        //    であり、 capacity を超えるタイミングの line が display 内にいることはありえない。
        //    wrap を考慮しても状況は変わらない。 wrap があるということは、 display 内の line 数が減る、つまり
        //    display size が小さくなることに等しい。よってやはりそれが capacity 以内に収まる限り、
        //    capacity を超えて pop される line は必ず display の外にあり、 pop して問題ないはず。
        //
        //    capacity = 2, display size = 1, original lines = 5 で、かつ1行目が wrap してたら？
        //    ある先頭行が display に含まれるかどうかは、その wrap に依存しちゃうから、実は事前に決定できないか。
        //    むしろ push をするタイミングが重要？
        //    つまり先頭行が wrap しまくってたら、スクロール時に push は発生しない？
        //    いや start_row の方だけ wrap してて、 end_row では新たな行が必要なケースってあるのでは。
        //    でもそのケースで capacity を超えることはない？
        //    そのケースでは、 display されてる line は必ず display size - 1 以下の数になる。つまり
        //    capacity は超えない。
        //    超える時って必ず start_row.deque_index > 0 ?

        // でもやっぱり capacity いっぱいの状態で先頭を表示しつつ push_back されたら見えてる部分が消えちゃう。
        // のでやはり push を単体で提供するインターフェイスは避けたい。
        // ただそもそも reader と分離してる以上、こいつが単体で正しい状態を確保しようとするのも無理があるか。
        // どんな形であれ適当な line を積まれたらおしまい。
        // 渡される line の内容や順序の正しさは呼び出し側の責務として、
        // キャッシュの管理といつ新しい line が必要になるかを Page が管理するようにしたい。

        debug_assert!(self.end_row.deque_index == self.deque.len() - 1);
        if self.deque.len() == self.deque.capacity() {
            self.deque.pop_front();
            self.start_row.deque_index -= 1;
            self.end_row.deque_index -= 1;
        }
        self.deque.push_back(line);
    }

    pub fn push_front(&mut self, line: PageLine<LineMeta>) {
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

    use crate::pager::{line::RowSpan, page::Page, PageLine};

    #[test]
    fn hold_lines_less_than_page_size() {
        let mut builder = Page::builder(3);
        builder.push_back(PageLine::new((), "abc".to_string(), 3));
        builder.push_back(PageLine::new((), "def".to_string(), 3));
        let page = builder.to_page().expect("build page");
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
        let mut page = builder.to_page().expect("build page");
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
