mod highlight;

use std::{cmp, collections::HashSet, io, mem, num::NonZeroUsize};

use crossterm::event::Event;

use crate::{
    document::Document,
    line::Row,
    page::{Direction, ScrollPlan},
    pager::{PageSnapshot, PageUpdate},
    screen::Screen,
    search::{MatchPosition, SearchState},
};

#[derive(Clone)]
struct RowRef {
    line_index: usize,
    wrap_index: usize,
}

impl PartialEq<Row> for RowRef {
    fn eq(&self, row: &Row) -> bool {
        self.line_index == row.line_index && self.wrap_index == row.wrap_index
    }
}

struct SearchStateRef {
    query: String,
    current: Option<MatchPosition>,
}

impl PartialEq<SearchState> for SearchStateRef {
    fn eq(&self, other: &SearchState) -> bool {
        &self.query == other.query.as_str() && self.current == other.current
    }
}

pub struct Renderer<S: Screen> {
    screen: S,
    last_section_header_start: Option<RowRef>,
    last_search: Option<SearchStateRef>,
    last_highlight_lines: HashSet<usize>,
    current_highlight_lines: HashSet<usize>,
    // 前回の描画時のハイライトがあった行一覧を保持
    // full 描画のときは特に使わない
    // scroll でかつ search state が変わっている時は、追加行以外の既存行も適宜更新
    //   前回のハイライト行 -> 再描画
    //   ?? それ以外の行は？　そこにも新しいハイライトがある可能性
    //   でも全行再描画なら、結局 full と変わらない
    //   実際には matches を探して、前回なかったのに今回あるケースのみ再描画になるか
    //   行継続の考慮も含めるとなかなか面倒そう
}

impl<S: Screen> Renderer<S> {
    pub fn new(screen: S) -> Self {
        Self {
            screen,
            last_section_header_start: None,
            last_search: None,
            last_highlight_lines: HashSet::new(),
            current_highlight_lines: HashSet::new(),
        }
    }

    #[cfg(test)]
    pub fn into_screen(self) -> S {
        self.screen
    }

    pub fn poll_event(&mut self, timeout: std::time::Duration) -> io::Result<Option<Event>> {
        self.screen.poll_event(timeout)
    }

    pub fn render(
        &mut self,
        doc: &mut Document,
        page: PageSnapshot,
        search: Option<&SearchState>,
        status_text: &str,
    ) -> io::Result<()> {
        let result = match page.last_update {
            PageUpdate::None => self.redraw_status_line(&page, status_text),
            PageUpdate::Full => self.redraw_full_page(doc, &page, search, status_text),
            PageUpdate::Scroll { up, n_rows } => {
                // todo: ヘッダーサイズが変わってたら一旦常に full redraw
                self.scroll(doc, &page, search, status_text, up, n_rows)
            }
        };
        self.last_section_header_start = page.section_header.get(0).map(|row| RowRef {
            line_index: row.line_index,
            wrap_index: row.wrap_index,
        });
        // XXX: 1行スクロールも含めて毎回コピーが走っちゃう。
        self.last_search = search.map(|s| SearchStateRef {
            query: s.query.as_str().to_string(),
            current: s.current,
        });
        self.last_highlight_lines = mem::take(&mut self.current_highlight_lines);
        result
    }

    fn redraw_status_line(&mut self, page: &PageSnapshot, status_text: &str) -> io::Result<()> {
        let status_y = page.global_header.len() + page.section_header.len() + page.content.len();
        self.screen.clear_row(status_y as u16)?;
        self.screen.write_at(status_y as u16, status_text)?;
        self.screen.flush()
    }

    fn redraw_full_page(
        &mut self,
        doc: &mut Document,
        page: &PageSnapshot,
        search: Option<&SearchState>,
        status_text: &str,
    ) -> io::Result<()> {
        self.screen.begin_sync()?;

        self.draw_rows_grouped(doc, page.global_header, search, 0)?;
        self.draw_rows_grouped(doc, page.section_header, search, page.global_header.len())?;
        self.draw_rows_grouped(
            doc,
            page.content,
            search,
            page.global_header.len() + page.section_header.len(),
        )?;

        // Clear any rows below content that may have stale content.
        let content_last_y =
            page.global_header.len() + page.section_header.len() + page.content.len();
        for y in content_last_y..page.height {
            self.screen.clear_row(y as u16)?;
        }

        self.screen.clear_row(content_last_y as u16)?;
        self.screen.write_at(content_last_y as u16, status_text)?;

        self.screen.end_sync()?;
        self.screen.flush()
    }

    fn scroll(
        &mut self,
        doc: &mut Document,
        page: &PageSnapshot,
        search: Option<&SearchState>,
        status_text: &str,
        is_up: bool,
        scroll_rows: usize,
    ) -> io::Result<()> {
        self.screen.begin_sync()?;

        let header_height = page.global_header.len() + page.section_header.len();
        let (from, to) = if scroll_rows > 0 {
            let direction = if is_up {
                Direction::Up
            } else {
                Direction::Down
            };
            self.screen.scroll_terminal(&ScrollPlan {
                terminal_scroll: NonZeroUsize::new(scroll_rows).unwrap(),
                direction,
            })?;

            let (from, to) = scroll_dirty_range(page.content, scroll_rows, direction);
            log::debug!("render header={header_height} scroll={scroll_rows} {from}..{to}");
            log::debug!("render {:?}", page.section_header);
            self.draw_rows_grouped(doc, &page.content[from..to], search, header_height + from)?;
            (from, to)
        } else {
            (0, 0)
        };

        log::debug!("render scroll {:?}", (from, to));
        let is_search_same = match (&self.last_search, search) {
            (Some(prev), Some(current)) => prev == current,
            (None, None) => true,
            _ => false,
        };
        // last_search != search なら、元々あった各行について higlight を更新。
        // todo: 直前のハイライト行位置を記憶し、再描画すべきもののみ再描画
        if !is_search_same {
            let (rest_from, rest_to) = if from == 0 {
                (to, page.content.len())
            } else {
                (0, from)
            };

            // last_highlight_lines
            // !last && !current => no draw
            // !last && current => draw
            // last && !current => draw
            // last && current => draw

            self.draw_rows_grouped2(
                doc,
                &page.content[rest_from..rest_to],
                search,
                header_height + rest_from,
            )?;
        }

        if !page.global_header.is_empty() {
            self.draw_rows_grouped(doc, page.global_header, search, 0)?;
        }

        self.draw_rows_grouped(doc, page.section_header, search, page.global_header.len())?;

        let content_last_y =
            page.global_header.len() + page.section_header.len() + page.content.len();
        self.screen.clear_row(content_last_y as u16)?;
        self.screen.write_at(content_last_y as u16, status_text)?;

        self.screen.end_sync()?;
        self.screen.flush()
    }

    /// Draw screen rows, grouping consecutive rows from the same logical line
    /// and writing them as a single continuous string so the terminal treats
    /// line-internal wraps as soft wraps.
    /// `screen_y` specifies the starting screen row position for drawing.
    fn draw_rows_grouped(
        &mut self,
        doc: &mut Document,
        rows: &[Row],
        search: Option<&SearchState>,
        screen_y: usize,
    ) -> io::Result<()> {
        let mut i = 0;
        while i < rows.len() {
            let line_idx = rows[i].line_index;
            let group_start = i;
            while i < rows.len() && rows[i].line_index == line_idx {
                i += 1;
            }
            // Clear each row in the group
            for j in group_start..i {
                self.screen.clear_row((j + screen_y) as u16)?;
            }
            // Write the combined text for this group as one continuous piece
            if let Some(line) = doc.line(line_idx) {
                let raw_range = rows[group_start].raw_range.start..rows[i - 1].raw_range.end;

                let matches = search.map(|sh| line.find_matches(&sh.query));
                match (search, matches) {
                    (Some(search), Some(matches)) if !matches.is_empty() => {
                        self.current_highlight_lines.insert(line_idx);

                        let current_match_index = search.current.and_then(|current| {
                            if current.line == line_idx {
                                Some(current.match_index)
                            } else {
                                None
                            }
                        });
                        let positions = highlight::build_highlight_positions(
                            &matches,
                            current_match_index,
                            line.plain_to_raw(),
                            line.raw().len(),
                        );
                        let text =
                            highlight::apply_highlight_to_range(line.raw(), raw_range, &positions);
                        self.screen
                            .write_at((group_start + screen_y) as u16, &text)?;
                    }
                    _ => {
                        self.screen
                            .write_at((group_start + screen_y) as u16, &line.raw()[raw_range])?;
                    }
                }
            }
        }
        Ok(())
    }

    fn draw_rows_grouped2(
        &mut self,
        doc: &mut Document,
        rows: &[Row],
        search: Option<&SearchState>,
        screen_y: usize,
    ) -> io::Result<()> {
        let mut i = 0;
        while i < rows.len() {
            let line_idx = rows[i].line_index;

            let group_start = i;
            while i < rows.len() && rows[i].line_index == line_idx {
                i += 1;
            }
            // Write the combined text for this group as one continuous piece
            if let Some(line) = doc.line(line_idx) {
                let raw_range = rows[group_start].raw_range.start..rows[i - 1].raw_range.end;

                let matches = search.map(|sh| line.find_matches(&sh.query));
                match (search, matches) {
                    (Some(search), Some(matches)) if !matches.is_empty() => {
                        self.current_highlight_lines.insert(line_idx);

                        // Clear each row in the group
                        for j in group_start..i {
                            self.screen.clear_row((j + screen_y) as u16)?;
                        }
                        let current_match_index = search.current.and_then(|current| {
                            if current.line == line_idx {
                                Some(current.match_index)
                            } else {
                                None
                            }
                        });
                        let positions = highlight::build_highlight_positions(
                            &matches,
                            current_match_index,
                            line.plain_to_raw(),
                            line.raw().len(),
                        );
                        let text =
                            highlight::apply_highlight_to_range(line.raw(), raw_range, &positions);
                        self.screen
                            .write_at((group_start + screen_y) as u16, &text)?;
                    }
                    _ => {
                        if !self.last_highlight_lines.contains(&line_idx) {
                            continue;
                        }
                        // Clear each row in the group
                        for j in group_start..i {
                            self.screen.clear_row((j + screen_y) as u16)?;
                        }
                        self.screen
                            .write_at((group_start + screen_y) as u16, &line.raw()[raw_range])?;
                    }
                }
            }
        }
        Ok(())
    }
}

/// Compute the range of rows that need redrawing after a scroll.
///
/// After terminal scroll shifts content, `scroll_rows` new rows appear at one
/// edge. This function returns the range extended to include adjacent existing
/// rows from the same logical line, so soft-wrap groups are drawn correctly.
fn scroll_dirty_range(rows: &[Row], scroll_rows: usize, direction: Direction) -> (usize, usize) {
    let len = rows.len();
    match direction {
        Direction::Down => {
            let new_start = len.saturating_sub(scroll_rows);
            let mut from = new_start;
            while from > 0 && rows[from - 1].line_index == rows[new_start].line_index {
                from -= 1;
            }
            (from, len)
        }
        Direction::Up => {
            let new_end = scroll_rows.min(len);
            let mut to = new_end;
            while to < len && rows[to].line_index == rows[new_end - 1].line_index {
                to += 1;
            }
            (0, to)
        }
    }
}
