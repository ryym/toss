// XXX: temporary
#![allow(unused)]

// todo: renderer のイメージを固めて、 Pager と Renderer をもとに App を実装させて、
// その後に細かいリファクタリングって感じかな
// あるいは先に既存 App の page を pager で置き換えて試せるとよりよい
// 一旦全部 draw full page にするのあり？　テストが通ることの確認ならそれで十分かも

use std::ops::Range;

use crate::{
    document::Document,
    line::{Line, Row},
    options::{Options, SectionOptions},
};

struct ViewportSize {
    width: usize,
    height: usize,
}

fn rows_from_lines(
    doc: &mut Document,
    width: usize,
    line_start: usize,
    line_end: usize,
) -> Vec<Row> {
    let mut rows = vec![];
    for i in line_start..line_end {
        if let Some(line) = doc.line(i) {
            rows.extend_from_slice(&line.wrap(width));
        }
    }
    rows
}

// 実装方針: global, section header ともに overlay 管理がいいのでは
// viewport は header 行含めて持つ方が、検索時にヒットを探してスクロールする処理が直感的になる。
// 検索はあくまで viewport のみで行い、正しい位置に動いた上で overlay を被せればいいのでは。
// ただし section header は global header の終了位置を把握し、それ以降は探索しない制御が必要。

pub struct PageSnapshot<'pager> {
    pub global_header: &'pager [Row],
    pub section_header: &'pager [Row],
    pub content: &'pager [Row],
    pub height: usize,
}

struct Pager {
    doc: Document,
    header: GlobalHeader,
    section: Section,
    viewport: Viewport,
}

impl Pager {
    fn new(mut doc: Document, size: &ViewportSize, options: Options) -> Self {
        let header = GlobalHeader::new(&mut doc, size, options.header);
        let section = Section::new(options.section, size, header.height());
        let viewport = Viewport::new(&mut doc, size);
        let mut pager = Pager {
            doc,
            header,
            section,
            viewport,
        };
        pager.jump_to(0, 0);
        pager
    }

    fn snapshot<'pager>(&'pager self) -> PageSnapshot<'pager> {
        PageSnapshot {
            global_header: self.header.rows(),
            section_header: self.section.header_rows(),
            content: &self.viewport.all_rows()[self.total_header_height()..],
            height: self.viewport.size().height,
        }
    }

    fn resize(&mut self, size: &ViewportSize) {
        self.header.resize(&mut self.doc, size);
        self.section
            .resize(&mut self.doc, size, self.header.height());
        self.viewport.resize(&mut self.doc, size);
    }

    fn total_header_height(&self) -> usize {
        self.header.height() + self.section.header_height()
    }

    fn jump_to(&mut self, mut line_index: usize, offset: usize) {
        // section header をセットする。
        self.section.resolve(&mut self.doc, line_index);

        // line_index がいずれかの global header 内にある場合は、 doc の先頭に移動。
        if self.header.contains(line_index) {
            line_index = 0;
        }
        // section header 内にある場合は、それが viewport の先頭に来るよう移動。
        // いずれの header 内でもない場合は、 line_index が可視領域の top に来るよう調整。
        let header_offset = if self.section.header_contains(line_index) {
            self.header.height()
        } else {
            self.total_header_height()
        };
        self.viewport
            .jump_to(&mut self.doc, line_index, offset + header_offset);
    }

    fn jump_to_end(&mut self) {
        // まずドキュメント末尾を基準に viewport をセットする。
        self.viewport.jump_to_end(&mut self.doc);
        // それをもとに対応する section header を被せる。
        let top_row_line_index = self.viewport.all_rows()[0].line_index;
        self.section.resolve(&mut self.doc, top_row_line_index);
        // overlay 内に次の section header がある場合は push-up する。
        self.push_up_section_header_if_needed();
    }

    fn scroll(&mut self, n_rows: i32) {
        if n_rows.unsigned_abs() as usize > self.viewport.size().height {
            panic!("scroll rows too big");
        }

        if n_rows < 0 {
            self.scroll_up((-n_rows) as usize);
        } else if n_rows > 0 {
            self.scroll_down(n_rows as usize);
        }
    }

    fn scroll_up(&mut self, n_rows: usize) {
        // viewport をスクロールする。
        self.viewport.scroll_up(&mut self.doc, n_rows);

        // 元々 section がない場合、上方向へのスクロールで新たに section が見つかることはありえないので、何もしない。
        let section_header_start = match self.section.start_line_index() {
            Some(idx) => idx,
            None => return,
        };
        // 新しい viewport 先頭が今の section header より手前にあるなら、より上の section を探してセットする。
        let top_index = self.viewport.all_rows()[self.header.height()].line_index;
        if top_index < section_header_start {
            self.section.resolve(&mut self.doc, top_index);
        }

        // 1つ前の section header がまだ overlay 領域にいる場合、新しい section header の位置を調整する。
        self.push_up_section_header_if_needed();
    }

    fn scroll_down(&mut self, n_rows: usize) {
        let prev_top_line = self.viewport.all_rows()[self.header.height()].line_index;

        // viewport をスクロールする。
        self.viewport.scroll_down(&mut self.doc, n_rows);
        let top_line = self.viewport.all_rows()[self.header.height()].line_index;

        // 移動範囲内に新たな section header があればそれに置き換える。
        self.section
            .resolve_if_found(&mut self.doc, prev_top_line..(top_line + 1));

        // overlay 領域にその次の section があるか探し、あれば section header 領域を狭める。
        self.push_up_section_header_if_needed();
    }

    // section header の overlay 配下に別の section header がないかを探し、
    // ある場合 (= section が切り替わっている途中の場合) にはその section が見えるよう
    // 今の section のオフセットを調整する。
    fn push_up_section_header_if_needed(&mut self) {
        let rows = self.viewport.all_rows();
        let overlay_height = self.header.height() + self.section.header_height_no_offset();
        let mut other_section_start = overlay_height;
        let rows_under_section_header = rows
            .iter()
            .enumerate()
            .take(overlay_height)
            .skip(self.header.height());
        for (i, row) in rows_under_section_header {
            if row.wrap_index != 0 {
                continue;
            }
            if let Some(line) = self.doc.line(row.line_index)
                && self.section.is_header(line)
            {
                other_section_start = i;
                break;
            }
        }
        let push_up = overlay_height.saturating_sub(other_section_start);
        self.section.push_up_start(push_up);
    }
}

struct GlobalHeader {
    n_lines: usize,
    rows: Vec<Row>,
}

impl GlobalHeader {
    fn new(doc: &mut Document, size: &ViewportSize, n_lines: usize) -> Self {
        let n_lines = n_lines.min(size.height - 1);
        let rows = Self::build_rows_static(doc, size, n_lines);
        GlobalHeader { n_lines, rows }
    }

    fn rows(&self) -> &[Row] {
        &self.rows
    }

    fn resize(&mut self, doc: &mut Document, size: &ViewportSize) {
        self.n_lines = self.n_lines.min(size.height - 1);
        self.rows = Self::build_rows_static(doc, size, self.n_lines);
    }

    fn height(&self) -> usize {
        self.rows.len()
    }

    fn contains(&self, line_index: usize) -> bool {
        line_index < self.height()
    }

    fn build_rows_static(doc: &mut Document, size: &ViewportSize, n_lines: usize) -> Vec<Row> {
        rows_from_lines(doc, size.width, 0, n_lines)
    }
}

struct SectionHeaderData {
    line_range: Range<usize>,
    rows: Vec<Row>,
    offset: usize,
}

struct SectionSizeConfig {
    min_line_index: usize,
    max_header_height: usize,
    width: usize,
}

struct Section {
    size: SectionSizeConfig,
    config: Option<SectionOptions>,
    header: Option<SectionHeaderData>,
}

impl Section {
    fn new(
        config: Option<SectionOptions>,
        size: &ViewportSize,
        global_header_height: usize,
    ) -> Self {
        Section {
            size: SectionSizeConfig {
                min_line_index: global_header_height,
                max_header_height: size.height - global_header_height - 1,
                width: size.width,
            },
            config,
            header: None,
        }
    }

    fn header_rows(&self) -> &[Row] {
        match &self.header {
            None => &[],
            Some(h) => &h.rows[h.offset..],
        }
    }

    fn start_line_index(&self) -> Option<usize> {
        self.header.as_ref().map(|h| h.line_range.start)
    }

    fn header_height(&self) -> usize {
        match &self.header {
            None => 0,
            Some(h) => h.rows.len() - h.offset,
        }
    }

    // offset を加味しない場合の section header height
    fn header_height_no_offset(&self) -> usize {
        match &self.header {
            None => 0,
            Some(h) => h.rows.len(),
        }
    }

    fn header_contains(&self, line_index: usize) -> bool {
        line_index < self.header_height()
    }

    // 行が section header のパターンにマッチするかを調べる。
    fn is_header(&self, line: &Line) -> bool {
        match &self.config {
            Some(cfg) => line.has_match(&cfg.pattern),
            None => false,
        }
    }

    fn resize(&mut self, doc: &mut Document, size: &ViewportSize, global_header_height: usize) {
        self.size = SectionSizeConfig {
            min_line_index: global_header_height,
            max_header_height: size.height - global_header_height - 1,
            width: size.width,
        };
        if let Some(h) = &mut self.header {
            h.rows = rows_from_lines(doc, size.width, h.line_range.start, h.line_range.end);
        }
    }

    // line_index に対応する直近の section header を探してセットする。
    // line_index が section header の先頭もしくは一部であるなら、その section header がセットされる。
    fn resolve(&mut self, doc: &mut Document, line_index: usize) {
        self.header = self.find_header(doc, self.size.min_line_index..line_index);
    }

    // line_index_range 内で section header を探し、見つかった場合のみそれをセットする。
    // 見つからなかった場合は今の section header が維持される。
    fn resolve_if_found(&mut self, doc: &mut Document, line_index_range: Range<usize>) {
        if let Some(header) = self.find_header(doc, line_index_range) {
            self.header = Some(header);
        }
    }

    fn find_header(&self, doc: &mut Document, range: Range<usize>) -> Option<SectionHeaderData> {
        let config = self.config.as_ref()?;
        if self.size.max_header_height == 0 {
            return None;
        }

        // 指定範囲内で最も end に近い section header を見つける。
        let mut nearest = None;
        if range.end > range.start {
            for i in (range.start..range.end).rev() {
                let line = doc.line(i)?;
                if line.has_match(&config.pattern) {
                    nearest = Some(i);
                    break;
                }
            }
        }
        let nearest = nearest?;

        let height = config.header_lines.min(self.size.max_header_height);
        let line_range = nearest..(nearest + height);
        Some(SectionHeaderData {
            rows: rows_from_lines(doc, self.size.width, line_range.start, line_range.end),
            line_range,
            offset: 0,
        })
    }

    // section header のうち指定行数分の先頭部分を上に押し上げ、見えなくする。
    fn push_up_start(&mut self, n_rows: usize) {
        if let Some(h) = &mut self.header {
            assert!(n_rows <= h.rows.len(), "push up section header invalid");
            h.offset = n_rows;
        }
    }
}

struct Viewport {
    size: ViewportSize,
    rows: Vec<Row>,
}

impl Viewport {
    fn new(doc: &mut Document, size: &ViewportSize) -> Self {
        let rows = build_rows_forward(doc, size.width, (0, 0), size.height);
        Viewport {
            size: ViewportSize {
                width: size.width,
                height: size.height,
            },
            rows,
        }
    }

    fn size(&self) -> &ViewportSize {
        &self.size
    }

    // overlay による不可視領域を含む viewport 全体の行一覧。 Pager の外では使わないようにしたい。
    fn all_rows(&self) -> &[Row] {
        &self.rows
    }

    fn resize(&mut self, doc: &mut Document, size: &ViewportSize) {
        self.rows = build_rows_forward(
            doc,
            size.width,
            (self.rows[0].line_index, self.rows[0].wrap_index),
            size.height,
        );
        self.size = ViewportSize {
            width: size.width,
            height: size.height,
        };
    }

    // 指定行数分を末尾から除去し、同じ行数分を先頭に追加する。
    fn scroll_up(&mut self, doc: &mut Document, n_rows: usize) {
        if n_rows == 0 || self.rows.is_empty() {
            return;
        }
        if n_rows > self.size.height {
            panic!("scroll rows too big");
        }
        // 指定行数分の新しい行を取得
        let first = self.rows[0].clone();
        let new_rows = build_rows_backward(doc, self.size.width, BuildFrom::Before(first), n_rows);

        // 新しく取得した行分だけ末尾から削除し、先頭に新しい行を追加
        let len = self.rows.len();
        self.rows.truncate(len - new_rows.len());
        self.rows.splice(0..0, new_rows);
    }

    // 指定行数分を先頭から除去し、同じ行数分を末尾に追加する。
    fn scroll_down(&mut self, doc: &mut Document, n_rows: usize) {
        if n_rows == 0 || self.rows.is_empty() {
            return;
        }
        if n_rows > self.size.height {
            panic!("scroll rows too big");
        }
        // 指定行数分の新しい行を取得
        let last = self.rows.last().unwrap().clone();
        let new_rows = build_rows_forward(
            doc,
            self.size.width,
            (last.line_index, last.wrap_index + 1),
            n_rows,
        );
        // 新しく取得した行分だけ先頭から削除し、末尾に新しい行を追加
        let remove_count = new_rows.len();
        self.rows.drain(0..remove_count);
        self.rows.extend(new_rows);
    }

    fn jump_to(&mut self, doc: &mut Document, line_index: usize, row_offset: usize) {
        let height = self.size.height;
        let rows_after_line = build_rows_forward(doc, self.size.width, (line_index, 0), height);

        let padding = height
            .saturating_sub(1)
            .min(row_offset.max(height.saturating_sub(rows_after_line.len())));
        let rows_before_line = build_rows_backward(
            doc,
            self.size.width,
            BuildFrom::Before(rows_after_line[0].clone()),
            padding,
        );

        let mut combined: Vec<Row> = rows_before_line;
        combined.extend(rows_after_line);
        combined.truncate(height);
        self.rows = combined;
    }

    fn jump_to_end(&mut self, doc: &mut Document) {
        let count = self.rows.len();
        self.rows = build_rows_backward(doc, self.size.width, BuildFrom::End, count);
    }
}

fn build_rows_forward(
    doc: &mut Document,
    width: usize,
    start: (usize, usize),
    count: usize,
) -> Vec<Row> {
    let mut rows = Vec::new();
    let (mut line_index, mut skip_wraps) = start;
    while rows.len() < count {
        let line = match doc.line(line_index) {
            Some(l) => l,
            None => break,
        };
        let line_rows: Vec<Row> = line
            .wrap(width)
            .into_iter()
            .skip(skip_wraps)
            .take(count - rows.len())
            .collect();
        rows.extend(line_rows);
        skip_wraps = 0;
        line_index += 1;
    }
    rows
}

enum BuildFrom {
    End,
    Before(Row),
}

fn build_rows_backward(
    doc: &mut Document,
    width: usize,
    from: BuildFrom,
    count: usize,
) -> Vec<Row> {
    let (mut line_index, mut from_wrap): (isize, Option<usize>) = match from {
        BuildFrom::End => ((doc.line_count() as isize) - 1, None),
        BuildFrom::Before(row) => {
            if row.wrap_index == 0 {
                if row.line_index == 0 {
                    return vec![];
                }
                ((row.line_index as isize) - 1, None)
            } else {
                (row.line_index as isize, Some(row.wrap_index - 1))
            }
        }
    };

    let mut rows = Vec::new();
    while rows.len() < count && line_index >= 0 {
        let line = match doc.line(line_index as usize) {
            Some(l) => l,
            None => break,
        };
        let mut line_rows = line.wrap(width);
        let skip_wraps_rev = match from_wrap {
            None => 0,
            Some(fw) => line_rows.len() - 1 - fw,
        };
        line_rows.reverse();
        let line_rows: Vec<Row> = line_rows
            .into_iter()
            .skip(skip_wraps_rev)
            .take(count - rows.len())
            .collect();
        rows.extend(line_rows);
        from_wrap = None;
        line_index -= 1;
    }
    rows.reverse();
    rows
}
