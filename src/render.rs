mod highlight;

use std::io;
use std::num::NonZeroUsize;

use crate::document::Document;
use crate::line::Row;
use crate::page::{Direction, Page, ScrollPlan};
use crate::pager::PageSnapshot;
use crate::screen::Screen;
use crate::search::SearchState;

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

/// Draw the status line, computing its position from the page layout.
pub fn draw_status_line<S: Screen>(screen: &mut S, page: &mut Page) -> io::Result<()> {
    with_sync(screen, |screen| draw_status_line_inner(screen, page))
}

/// Resolve and redraw header rows at the top of the screen.
fn redraw_header<S: Screen>(
    screen: &mut S,
    page: &mut Page,
    search: Option<&SearchState>,
) -> io::Result<()> {
    let header = page.resolve_header();
    if header.height() > 0 {
        draw_rows_grouped(screen, &mut page.doc, header.rows(), search, 0)?;
    }
    Ok(())
}

fn draw_status_line_inner<S: Screen>(screen: &mut S, page: &mut Page) -> io::Result<()> {
    let header = page.resolve_header();
    let status_y = (header.fixed_height() + page.viewport.rows().len()) as u16;
    screen.clear_row(status_y)?;
    screen.write_at(status_y, page.status.render())?;
    Ok(())
}

/// Render a full page (used on initial draw and resize).
pub fn draw_full_page<S: Screen>(
    screen: &mut S,
    page: &mut Page,
    search: Option<&SearchState>,
) -> io::Result<()> {
    with_sync(screen, |screen| {
        let header = page.resolve_header_synced();
        let header_height = header.height();

        // Draw header rows at the top of the screen.
        if header_height > 0 {
            draw_rows_grouped(screen, &mut page.doc, header.rows(), search, 0)?;
        }

        // Draw viewport rows below the header, skipping overlaid rows.
        let visible_rows = page.viewport.visible_rows();
        draw_rows_grouped(screen, &mut page.doc, visible_rows, search, header_height)?;

        // Clear any rows below content that may have stale content.
        for y in visible_rows.len()..page.viewport.visible_height() {
            screen.clear_row((y + header_height) as u16)?;
        }
        draw_status_line_inner(screen, page)
    })
}

pub fn draw_full_page2<S: Screen>(
    screen: &mut S,
    mut doc: &mut Document,
    page: &PageSnapshot,
    search: Option<&SearchState>,
) -> io::Result<()> {
    with_sync(screen, |screen| {
        draw_rows_grouped(screen, &mut doc, page.global_header, search, 0)?;
        draw_rows_grouped(
            screen,
            &mut doc,
            page.section_header,
            search,
            page.global_header.len(),
        )?;
        draw_rows_grouped(
            screen,
            &mut doc,
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

        // draw_status_line_inner(screen, page)
        screen.clear_row(content_last_y as u16)?;
        screen.write_at(content_last_y as u16, ":")?;
        Ok(())
    })
}

/// Apply incremental rendering for a viewport jump (e.g., n/N search jump).
///
/// When a jump scrolls the viewport by a small amount, we can reuse the
/// overlapping content via terminal scroll instead of redrawing everything.
/// Additionally redraws rows in the overlap area whose highlight state changed.
pub fn apply_jump_scroll<S: Screen>(
    screen: &mut S,
    page: &mut Page,
    scroll_rows: usize,
    direction: Direction,
    search: Option<&SearchState>,
    highlight_dirty_lines: &[usize],
) -> io::Result<()> {
    let plan = ScrollPlan {
        terminal_scroll: NonZeroUsize::new(scroll_rows).unwrap(),
        direction,
    };
    with_sync(screen, |screen| {
        apply_scroll_inner(screen, &plan, page, search)?;

        // In the overlap area (rows not scrolled in), redraw any rows whose
        // highlight state changed (e.g., old/new current match line for n/N).
        if !highlight_dirty_lines.is_empty() {
            let header_height = page.resolve_header().height();
            let visible_rows = page.viewport.visible_rows();

            let (dirty_from, dirty_to) = scroll_dirty_range(visible_rows, scroll_rows, direction);
            let overlap_range = match direction {
                Direction::Down => 0..dirty_from,
                Direction::Up => dirty_to..visible_rows.len(),
            };
            let dirty_rows: Vec<Row> = visible_rows[overlap_range]
                .iter()
                .filter(|r| highlight_dirty_lines.contains(&r.line_index))
                .cloned()
                .collect();
            if !dirty_rows.is_empty() {
                draw_dirty_rows_at_positions(
                    screen,
                    &mut page.doc,
                    visible_rows,
                    &dirty_rows,
                    search,
                    header_height,
                )?;
            }
        }
        Ok(())
    })
}

/// Redraw only the rows whose search highlights have changed.
///
/// `old_match_lines` contains line indices that had highlights before the change.
/// This function redraws rows belonging to those lines (to clear old highlights)
/// plus rows belonging to lines that match the current search (to draw new highlights).
pub fn draw_search_highlight_update<S: Screen>(
    screen: &mut S,
    page: &mut Page,
    search: Option<&SearchState>,
    old_match_lines: &[usize],
) -> io::Result<()> {
    with_sync(screen, |screen| {
        let header = page.resolve_header();
        let header_height = header.height();

        // Redraw header rows that need highlight updates.
        if header_height > 0 {
            let dirty_header =
                filter_dirty_rows(header.rows(), &mut page.doc, search, old_match_lines);
            if !dirty_header.is_empty() {
                draw_rows_grouped(screen, &mut page.doc, &dirty_header, search, 0)?;
            }
        }

        // Redraw viewport rows that need highlight updates.
        let visible_rows = page.viewport.visible_rows();
        let dirty_rows = filter_dirty_rows(visible_rows, &mut page.doc, search, old_match_lines);
        if !dirty_rows.is_empty() {
            draw_dirty_rows_at_positions(
                screen,
                &mut page.doc,
                visible_rows,
                &dirty_rows,
                search,
                header_height,
            )?;
        }

        draw_status_line_inner(screen, page)
    })
}

/// Filter rows to only those belonging to lines that need highlight redraw.
fn filter_dirty_rows(
    rows: &[Row],
    doc: &mut Document,
    search: Option<&SearchState>,
    old_match_lines: &[usize],
) -> Vec<Row> {
    let mut dirty = Vec::new();
    let mut last_line = None;
    let mut last_dirty = false;

    for row in rows {
        if last_line == Some(row.line_index) {
            // Same logical line as previous row: same dirty status.
            if last_dirty {
                dirty.push(row.clone());
            }
            continue;
        }
        last_line = Some(row.line_index);

        let is_dirty = old_match_lines.contains(&row.line_index)
            || search
                .and_then(|s| {
                    doc.line(row.line_index)
                        .map(|line| line.has_match(&s.query))
                })
                .unwrap_or(false);

        last_dirty = is_dirty;
        if is_dirty {
            dirty.push(row.clone());
        }
    }
    dirty
}

/// Compute the range of rows that need redrawing after a scroll.
///
/// After terminal scroll shifts content, `scroll_rows` new rows appear at one
/// edge. This function returns the range extended to include adjacent existing
/// rows from the same logical line, so soft-wrap groups are drawn correctly.
fn scroll_dirty_range(
    visible_rows: &[Row],
    scroll_rows: usize,
    direction: Direction,
) -> (usize, usize) {
    let len = visible_rows.len();
    match direction {
        Direction::Down => {
            let new_start = len.saturating_sub(scroll_rows);
            let mut from = new_start;
            while from > 0
                && visible_rows[from - 1].line_index == visible_rows[new_start].line_index
            {
                from -= 1;
            }
            (from, len)
        }
        Direction::Up => {
            let new_end = scroll_rows.min(len);
            let mut to = new_end;
            while to < len && visible_rows[to].line_index == visible_rows[new_end - 1].line_index {
                to += 1;
            }
            (0, to)
        }
    }
}

/// Draw dirty rows at their correct screen positions within visible_rows.
/// Each contiguous group of dirty rows from the same logical line is drawn
/// as a single write via draw_rows_grouped.
fn draw_dirty_rows_at_positions<S: Screen>(
    screen: &mut S,
    doc: &mut Document,
    visible_rows: &[Row],
    dirty_rows: &[Row],
    search: Option<&SearchState>,
    header_height: usize,
) -> io::Result<()> {
    // Build a set of dirty rows for lookup.
    let dirty_set: std::collections::HashSet<Row> = dirty_rows.iter().cloned().collect();

    // Walk visible_rows and find contiguous groups of dirty rows.
    let mut i = 0;
    while i < visible_rows.len() {
        if !dirty_set.contains(&visible_rows[i]) {
            i += 1;
            continue;
        }
        // Found a dirty row. Collect the contiguous group from the same line.
        let group_start = i;
        let line_idx = visible_rows[i].line_index;
        while i < visible_rows.len()
            && visible_rows[i].line_index == line_idx
            && dirty_set.contains(&visible_rows[i])
        {
            i += 1;
        }
        let group = &visible_rows[group_start..i];
        draw_rows_grouped(screen, doc, group, search, header_height + group_start)?;
    }
    Ok(())
}

/// Apply a scroll plan with incremental rendering.
///
/// After terminal scroll, we determine the "dirty range": the new rows plus
/// any adjacent existing rows that belong to the same logical line. The dirty
/// range is then redrawn with grouped continuous writes to maintain soft wraps.
pub fn apply_scroll<S: Screen>(
    screen: &mut S,
    plan: &ScrollPlan,
    page: &mut Page,
    search: Option<&SearchState>,
) -> io::Result<()> {
    with_sync(screen, |screen| {
        apply_scroll_inner(screen, plan, page, search)
    })
}

/// Core scroll rendering shared by apply_scroll and apply_jump_scroll.
///
/// Issues a terminal scroll command, redraws the newly exposed rows plus
/// any adjacent rows from the same logical line, then redraws the header
/// and status line.
fn apply_scroll_inner<S: Screen>(
    screen: &mut S,
    plan: &ScrollPlan,
    page: &mut Page,
    search: Option<&SearchState>,
) -> io::Result<()> {
    // When overlay covers the entire viewport, there are no content rows to scroll.
    // Just redraw the header and status line.
    if page.viewport.visible_height() == 0 {
        redraw_header(screen, page, search)?;
        return draw_status_line_inner(screen, page);
    }

    let header_height = page.resolve_header().height();

    // Terminal scroll shifts the entire screen (including header and status line).
    // We redraw the header and status line after scrolling.
    screen.scroll_terminal(plan)?;

    let visible_rows = page.viewport.visible_rows();
    let n_scroll = plan.terminal_scroll.get();

    let (draw_from, draw_to) = scroll_dirty_range(visible_rows, n_scroll, plan.direction);

    draw_rows_grouped(
        screen,
        &mut page.doc,
        &visible_rows[draw_from..draw_to],
        search,
        header_height + draw_from,
    )?;

    // Redraw header (scrolled away by terminal scroll).
    redraw_header(screen, page, search)?;

    draw_status_line_inner(screen, page)
}
