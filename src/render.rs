mod highlight;

use std::io;
use std::num::NonZeroUsize;

use crate::document::Document;
use crate::page::Page;
use crate::screen::Screen;
use crate::search::SearchState;
use crate::viewport::{Direction, ScreenRow, ScrollPlan};

/// Draw screen rows, grouping consecutive rows from the same logical line
/// and writing them as a single continuous string so the terminal treats
/// line-internal wraps as soft wraps.
/// `screen_y` specifies the starting screen row position for drawing.
fn draw_rows_grouped<S: Screen>(
    screen: &mut S,
    doc: &mut Document,
    rows: &[ScreenRow],
    width: usize,
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
            let first_wrap = rows[group_start].wrap_index;
            let last_wrap = rows[i - 1].wrap_index;
            let raw_range = line.wrap_rows_range(width, first_wrap, last_wrap + 1);

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

/// Draw the status line, computing its position from the page layout.
pub fn draw_status_line<S: Screen>(screen: &mut S, page: &mut Page) -> io::Result<()> {
    screen.begin_sync()?;
    draw_status_line_no_flush(screen, page)?;
    screen.end_sync()?;
    screen.flush()
}

fn draw_status_line_no_flush<S: Screen>(screen: &mut S, page: &mut Page) -> io::Result<()> {
    let header_height = page.resolve_header().len();
    let overlay = page.section_overlay();
    let status_y = (header_height + page.viewport.rows().len() - overlay) as u16;
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
    screen.begin_sync()?;
    let width = page.viewport.width();
    let header_rows = page.resolve_header();
    let header_height = header_rows.len();
    let overlay = page.section_overlay();

    // Draw header rows at the top of the screen.
    if header_height > 0 {
        draw_rows_grouped(screen, &mut page.doc, &header_rows, width, search, 0)?;
    }

    // Draw viewport rows below the header, skipping overlaid rows.
    let rows = page.viewport.rows();
    let skip = overlay.min(rows.len());
    let visible_rows = &rows[skip..];
    draw_rows_grouped(
        screen,
        &mut page.doc,
        visible_rows,
        width,
        search,
        header_height,
    )?;

    // Clear any rows below content that may have stale content.
    let visible_capacity = page.viewport.height().saturating_sub(skip);
    for y in visible_rows.len()..visible_capacity {
        screen.clear_row((y + header_height) as u16)?;
    }
    draw_status_line_no_flush(screen, page)?;

    screen.end_sync()?;
    screen.flush()
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
    screen.begin_sync()?;

    let width = page.viewport.width();
    let overlay = page.section_overlay();

    let visible_height = page.viewport.height().saturating_sub(overlay);
    if visible_height == 0 {
        let header_rows = page.resolve_header();
        if !header_rows.is_empty() {
            draw_rows_grouped(screen, &mut page.doc, &header_rows, width, search, 0)?;
        }
        draw_status_line_no_flush(screen, page)?;
        screen.end_sync()?;
        return screen.flush();
    }

    let header_height = page.resolve_header().len();

    // Issue terminal scroll to shift existing content in-place.
    let plan = ScrollPlan {
        terminal_scroll: NonZeroUsize::new(scroll_rows).unwrap(),
        direction,
    };
    screen.scroll_terminal(&plan)?;

    // Work with visible rows (viewport rows after skipping overlay).
    let rows = page.viewport.rows();
    let skip = overlay.min(rows.len());
    let visible_rows = &rows[skip..];
    let content_height = visible_rows.len();

    let (scroll_draw_from, scroll_draw_to) =
        scroll_dirty_range(visible_rows, scroll_rows, direction);

    // Draw the newly scrolled-in rows.
    draw_rows_grouped(
        screen,
        &mut page.doc,
        &visible_rows[scroll_draw_from..scroll_draw_to],
        width,
        search,
        header_height + scroll_draw_from,
    )?;

    // In the overlap area (rows not scrolled in), redraw any rows whose
    // highlight state changed (e.g., old/new current match line for n/N).
    if !highlight_dirty_lines.is_empty() {
        let overlap_range = match direction {
            Direction::Down => 0..scroll_draw_from,
            Direction::Up => scroll_draw_to..content_height,
        };
        let dirty_groups: Vec<ScreenRow> = visible_rows[overlap_range]
            .iter()
            .filter(|r| highlight_dirty_lines.contains(&r.line_index))
            .copied()
            .collect();
        if !dirty_groups.is_empty() {
            draw_dirty_rows_at_positions(
                screen,
                &mut page.doc,
                visible_rows,
                &dirty_groups,
                width,
                search,
                header_height,
            )?;
        }
    }

    // Redraw header (shifted by terminal scroll).
    let header_rows = page.resolve_header();
    if !header_rows.is_empty() {
        draw_rows_grouped(screen, &mut page.doc, &header_rows, width, search, 0)?;
    }

    draw_status_line_no_flush(screen, page)?;

    screen.end_sync()?;
    screen.flush()
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
    screen.begin_sync()?;

    let width = page.viewport.width();
    let header_rows = page.resolve_header();
    let header_height = header_rows.len();
    let overlay = page.section_overlay();

    // Redraw header rows that need highlight updates.
    if header_height > 0 {
        let dirty_header = filter_dirty_rows(&header_rows, &mut page.doc, search, old_match_lines);
        if !dirty_header.is_empty() {
            draw_rows_grouped(screen, &mut page.doc, &dirty_header, width, search, 0)?;
        }
    }

    // Redraw viewport rows that need highlight updates.
    let rows = page.viewport.rows();
    let skip = overlay.min(rows.len());
    let visible_rows = &rows[skip..];
    let dirty_rows = filter_dirty_rows(visible_rows, &mut page.doc, search, old_match_lines);
    if !dirty_rows.is_empty() {
        // We need to draw each dirty group at its correct screen position.
        // Since dirty_rows may be non-contiguous, draw them group by group.
        draw_dirty_rows_at_positions(
            screen,
            &mut page.doc,
            visible_rows,
            &dirty_rows,
            width,
            search,
            header_height,
        )?;
    }

    draw_status_line_no_flush(screen, page)?;

    screen.end_sync()?;
    screen.flush()
}

/// Filter rows to only those belonging to lines that need highlight redraw.
fn filter_dirty_rows(
    rows: &[ScreenRow],
    doc: &mut Document,
    search: Option<&SearchState>,
    old_match_lines: &[usize],
) -> Vec<ScreenRow> {
    let mut dirty = Vec::new();
    let mut last_line = None;
    let mut last_dirty = false;

    for row in rows {
        if last_line == Some(row.line_index) {
            // Same logical line as previous row: same dirty status.
            if last_dirty {
                dirty.push(*row);
            }
            continue;
        }
        last_line = Some(row.line_index);

        let is_dirty = old_match_lines.contains(&row.line_index)
            || search
                .and_then(|s| {
                    doc.line(row.line_index)
                        .map(|line| !line.find_matches(&s.query).is_empty())
                })
                .unwrap_or(false);

        last_dirty = is_dirty;
        if is_dirty {
            dirty.push(*row);
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
    visible_rows: &[ScreenRow],
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
    visible_rows: &[ScreenRow],
    dirty_rows: &[ScreenRow],
    width: usize,
    search: Option<&SearchState>,
    header_height: usize,
) -> io::Result<()> {
    // Build a set of dirty rows for lookup.
    let dirty_set: std::collections::HashSet<ScreenRow> = dirty_rows.iter().copied().collect();

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
        draw_rows_grouped(
            screen,
            doc,
            group,
            width,
            search,
            header_height + group_start,
        )?;
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
    screen.begin_sync()?;
    apply_scroll_no_flush(screen, plan, page, search)?;
    screen.end_sync()?;
    screen.flush()
}

fn apply_scroll_no_flush<S: Screen>(
    screen: &mut S,
    plan: &ScrollPlan,
    page: &mut Page,
    search: Option<&SearchState>,
) -> io::Result<()> {
    let width = page.viewport.width();
    let overlay = page.section_overlay();

    // When overlay covers the entire viewport, there are no content rows to scroll.
    // Just redraw the header and status line.
    let viewport_height = page.viewport.height();
    let visible_height = viewport_height.saturating_sub(overlay);
    if visible_height == 0 {
        let header_rows = page.resolve_header();
        if !header_rows.is_empty() {
            draw_rows_grouped(screen, &mut page.doc, &header_rows, width, search, 0)?;
        }
        return draw_status_line_no_flush(screen, page);
    }

    let header_height = page.resolve_header().len();

    // Terminal scroll shifts the entire screen (including header and status line).
    // We redraw the header and status line after scrolling.
    screen.scroll_terminal(plan)?;

    // Work with visible rows (viewport rows after skipping overlay).
    let rows = page.viewport.rows();
    let skip = overlay.min(rows.len());
    let visible_rows = &rows[skip..];
    let n_scroll = plan.terminal_scroll.get();

    let (draw_from, draw_to) = scroll_dirty_range(visible_rows, n_scroll, plan.direction);

    draw_rows_grouped(
        screen,
        &mut page.doc,
        &visible_rows[draw_from..draw_to],
        width,
        search,
        header_height + draw_from,
    )?;

    // Redraw header (scrolled away by terminal scroll).
    let header_rows = page.resolve_header();
    if !header_rows.is_empty() {
        draw_rows_grouped(screen, &mut page.doc, &header_rows, width, search, 0)?;
    }

    draw_status_line_no_flush(screen, page)
}
