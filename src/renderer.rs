mod highlight;

use std::{io, num::NonZeroUsize};

use crossterm::event::Event;

use crate::{
    document::Document,
    line::Row,
    page::{Direction, ScrollPlan},
    pager::PageSnapshot,
    screen::Screen,
    search::SearchState,
};

pub struct Renderer<S: Screen> {
    screen: S,
}

impl<S: Screen> Renderer<S> {
    pub fn new(screen: S) -> Self {
        Self { screen }
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
        with_sync(&mut self.screen, |screen| {
            draw_rows_grouped(screen, doc, page.global_header, search, 0)?;
            draw_rows_grouped(
                screen,
                doc,
                page.section_header,
                search,
                page.global_header.len(),
            )?;
            draw_rows_grouped(
                screen,
                doc,
                page.content,
                search,
                page.global_header.len() + page.section_header.len(),
            )?;

            // Clear any rows below content that may have stale content.
            let content_last_y =
                page.global_header.len() + page.section_header.len() + page.content.len();
            for y in content_last_y..page.height {
                screen.clear_row(y as u16)?;
            }

            screen.clear_row(content_last_y as u16)?;
            screen.write_at(content_last_y as u16, status_text)?;
            Ok(())
        })
    }
}

/// Wrap a rendering operation in synchronized output and flush.
fn with_sync<S: Screen>(
    screen: &mut S,
    f: impl FnOnce(&mut S) -> io::Result<()>,
) -> io::Result<()> {
    screen.begin_sync()?;
    f(screen)?;
    screen.end_sync()?;
    screen.flush()
}

/// Draw screen rows, grouping consecutive rows from the same logical line
/// and writing them as a single continuous string so the terminal treats
/// line-internal wraps as soft wraps.
/// `screen_y` specifies the starting screen row position for drawing.
fn draw_rows_grouped<S: Screen>(
    screen: &mut S,
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
            screen.clear_row((j + screen_y) as u16)?;
        }
        // Write the combined text for this group as one continuous piece
        if let Some(line) = doc.line(line_idx) {
            let raw_range = rows[group_start].raw_range.start..rows[i - 1].raw_range.end;

            let matches = search.map(|sh| line.find_matches(&sh.query));
            match (search, matches) {
                (Some(search), Some(matches)) if !matches.is_empty() => {
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
                    screen.write_at((group_start + screen_y) as u16, &text)?;
                }
                _ => {
                    screen.write_at((group_start + screen_y) as u16, &line.raw()[raw_range])?;
                }
            }
        }
    }
    Ok(())
}
