use std::io;
use std::time::Duration;

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use regex::Regex;

use crate::document::Document;
use crate::line_editor::LineEditor;
use crate::page::Page;
use crate::render;
use crate::screen::Screen;
use crate::scroll::ScrollPhysics;
use crate::search::{self, MatchPosition, SearchDirection, SearchState};
use crate::viewport::{Direction, ScreenRow};

const FRAME_DURATION_ANIMATING: Duration = Duration::from_millis(8);
const FRAME_DURATION_IDLE: Duration = Duration::from_millis(50);

/// What kind of redraw is needed for the next frame.
enum RedrawState {
    /// No redraw needed.
    None,
    /// Full page redraw (resize, scroll jump, cancel search, etc.).
    Full,
    /// Only search highlight changes — redraw affected rows only.
    SearchHighlight {
        /// Line indices that had highlights before the change.
        old_match_lines: Vec<usize>,
    },
    /// Viewport jumped with overlap — use incremental scroll + highlight update.
    JumpScroll {
        scroll_rows: usize,
        direction: Direction,
        /// Lines whose highlight style changed (old/new current match).
        highlight_dirty_lines: Vec<usize>,
    },
}

/// Current input mode of the application.
enum AppMode {
    View,
    Search {
        direction: SearchDirection,
        editor: LineEditor,
        /// Top line before search started, for restoring on cancel.
        saved_top_line: usize,
        /// Live search result updated on each keystroke.
        preview: Option<SearchState>,
    },
}

pub struct App<S> {
    screen: S,
    page: Page,
    mode: AppMode,
    search: Option<SearchState>,
    scroll_physics: ScrollPhysics,
    instant_scroll: bool,
    redraw: RedrawState,
}

impl<S: Screen> App<S> {
    pub fn new(screen: S, page: Page) -> io::Result<Self> {
        let (_, h) = screen.size()?;
        let mut scroll_physics = ScrollPhysics::new();
        scroll_physics.configure(h as usize);
        Ok(Self {
            screen,
            page,
            mode: AppMode::View,
            search: None,
            scroll_physics,
            instant_scroll: false,
            redraw: RedrawState::Full,
        })
    }

    #[cfg(test)]
    pub fn set_instant_scroll(&mut self) {
        self.instant_scroll = true;
    }

    #[cfg(test)]
    pub fn into_screen(self) -> S {
        self.screen
    }

    pub fn run(&mut self) -> io::Result<()> {
        // Initial draw
        let (_, h) = self.screen.size()?;
        self.page.sync_section_for_redraw(h as usize);
        let search = active_search(&self.mode, &self.search);
        render::draw_full_page(&mut self.screen, &mut self.page, search)?;
        self.redraw = RedrawState::None;

        loop {
            // 1. Poll input
            let timeout = if self.scroll_physics.is_active() {
                FRAME_DURATION_ANIMATING
            } else {
                FRAME_DURATION_IDLE
            };

            if let Some(event) = self.screen.poll_event(timeout)? {
                match event {
                    Event::Key(key) => {
                        if self.handle_key(key)? {
                            return Ok(());
                        }
                    }
                    Event::Resize(w, h) => {
                        log::debug!("Resize: {w}x{h}");
                        self.page.resize(w as usize, h as usize);
                        self.scroll_physics.configure(h as usize);
                        self.redraw = RedrawState::Full;
                    }
                    _ => {}
                }
            }

            // 2. Update animation
            self.update_animation()?;

            // 3. Render if needed
            match std::mem::replace(&mut self.redraw, RedrawState::None) {
                RedrawState::Full => {
                    let (_, h) = self.screen.size()?;
                    self.page.sync_section_for_redraw(h as usize);
                    let search = active_search(&self.mode, &self.search);
                    render::draw_full_page(&mut self.screen, &mut self.page, search)?;
                }
                RedrawState::SearchHighlight { old_match_lines } => {
                    let search = active_search(&self.mode, &self.search);
                    render::draw_search_highlight_update(
                        &mut self.screen,
                        &mut self.page,
                        search,
                        &old_match_lines,
                    )?;
                }
                RedrawState::JumpScroll {
                    scroll_rows,
                    direction,
                    highlight_dirty_lines,
                } => {
                    let search = active_search(&self.mode, &self.search);
                    render::apply_jump_scroll(
                        &mut self.screen,
                        &mut self.page,
                        scroll_rows,
                        direction,
                        search,
                        &highlight_dirty_lines,
                    )?;
                }
                RedrawState::None => {
                    if self.page.status.is_dirty() {
                        render::draw_status_line(&mut self.screen, &mut self.page)?;
                    }
                }
            }
        }
    }

    /// Returns true if the app should quit.
    fn handle_key(&mut self, key: KeyEvent) -> io::Result<bool> {
        log::debug!("Key: {key:?}");
        match self.mode {
            AppMode::View => self.handle_key_view(key),
            AppMode::Search { .. } => {
                self.handle_key_search(key);
                Ok(false)
            }
        }
    }

    fn handle_key_view(&mut self, key: KeyEvent) -> io::Result<bool> {
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => return Ok(true),
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                return Ok(true);
            }
            KeyCode::Char('j') | KeyCode::Down => {
                self.scroll_immediate(1)?;
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.scroll_immediate(-1)?;
            }
            KeyCode::Char('d') => {
                self.scroll_animated(self.page.viewport.height() as isize / 2)?;
            }
            KeyCode::Char('u') => {
                self.scroll_animated(-(self.page.viewport.height() as isize / 2))?;
            }
            KeyCode::Char('f') | KeyCode::Char(' ') => {
                self.scroll_animated(self.page.viewport.height() as isize)?;
            }
            KeyCode::Char('b') => {
                self.scroll_animated(-(self.page.viewport.height() as isize))?;
            }
            KeyCode::Char('g') => {
                self.scroll_physics.stop();
                if self.page.viewport.jump_to(&mut self.page.doc, 0) {
                    self.redraw = RedrawState::Full;
                }
            }
            KeyCode::Char('G') => {
                self.scroll_physics.stop();
                if self.page.viewport.jump_to_end(&mut self.page.doc) {
                    self.redraw = RedrawState::Full;
                }
            }
            KeyCode::Char('/') => {
                self.enter_search_mode(SearchDirection::Forward);
            }
            KeyCode::Char('?') => {
                self.enter_search_mode(SearchDirection::Backward);
            }
            KeyCode::Char('n') => {
                self.jump_to_next_match(false);
            }
            KeyCode::Char('N') => {
                self.jump_to_next_match(true);
            }
            _ => {}
        }
        Ok(false)
    }

    fn enter_search_mode(&mut self, direction: SearchDirection) {
        log::debug!("Enter search mode: {direction:?}");
        self.scroll_physics.stop();
        let saved_top_line = self.page.viewport.top_line_index();
        let editor = LineEditor::new();
        self.page.status.set_content(format!(
            "{}{}",
            direction.prompt(),
            editor.input_with_cursor()
        ));
        self.mode = AppMode::Search {
            direction,
            editor,
            saved_top_line,
            preview: None,
        };
    }

    fn exit_search_mode(&mut self) {
        log::debug!("Exit search mode");
        self.mode = AppMode::View;
        self.page.status.set_content(":".to_string());
    }

    /// Cancel search: discard preview and restore the original scroll position.
    fn cancel_search(&mut self) {
        if let AppMode::Search { saved_top_line, .. } = &self.mode {
            let top = *saved_top_line;
            self.page.viewport.jump_to(&mut self.page.doc, top);
            self.redraw = RedrawState::Full;
        }
        self.exit_search_mode();
    }

    /// Update the search preview based on current input.
    fn update_search_preview(&mut self) {
        let (input, direction, saved_top_line) = match &self.mode {
            AppMode::Search {
                editor,
                direction,
                saved_top_line,
                ..
            } => (editor.input().to_string(), *direction, *saved_top_line),
            _ => return,
        };

        // Collect lines with current highlights before making changes.
        let old_match_lines = self.collect_visible_match_lines();

        if input.is_empty() {
            // No query: clear preview and restore position.
            if let AppMode::Search { preview, .. } = &mut self.mode {
                *preview = None;
            }
            self.page
                .viewport
                .jump_to(&mut self.page.doc, saved_top_line);
            self.redraw = RedrawState::Full;
            return;
        }

        let re = regex::Regex::new(&regex::escape(&input)).unwrap();
        let matched = search::find_next_match(&mut self.page.doc, &re, saved_top_line, direction);
        log::debug!("Search preview: query={input:?}, result={matched:?}");

        let scrolled = if let Some(ref pos) = matched {
            self.page.jump_to_visible(pos.line)
        } else {
            false
        };

        if let AppMode::Search { preview, .. } = &mut self.mode {
            *preview = Some(SearchState {
                query: re,
                direction,
                current: matched,
            });
        }

        if scrolled {
            self.redraw = RedrawState::Full;
        } else {
            self.redraw = RedrawState::SearchHighlight { old_match_lines };
        }
    }

    fn handle_key_search(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Enter => {
                self.submit_search();
            }
            KeyCode::Esc => {
                self.cancel_search();
            }
            KeyCode::Backspace => {
                if let AppMode::Search {
                    direction, editor, ..
                } = &mut self.mode
                {
                    if editor.input().is_empty() {
                        self.cancel_search();
                        return;
                    }
                    editor.backspace();
                    self.page.status.set_content(format!(
                        "{}{}",
                        direction.prompt(),
                        editor.input_with_cursor()
                    ));
                }
                self.update_search_preview();
            }
            KeyCode::Char(ch) => {
                if let AppMode::Search {
                    direction, editor, ..
                } = &mut self.mode
                {
                    editor.insert(ch);
                    self.page.status.set_content(format!(
                        "{}{}",
                        direction.prompt(),
                        editor.input_with_cursor()
                    ));
                }
                self.update_search_preview();
            }
            KeyCode::Left => {
                if let AppMode::Search {
                    direction, editor, ..
                } = &mut self.mode
                {
                    editor.move_left();
                    self.page.status.set_content(format!(
                        "{}{}",
                        direction.prompt(),
                        editor.input_with_cursor()
                    ));
                }
            }
            KeyCode::Right => {
                if let AppMode::Search {
                    direction, editor, ..
                } = &mut self.mode
                {
                    editor.move_right();
                    self.page.status.set_content(format!(
                        "{}{}",
                        direction.prompt(),
                        editor.input_with_cursor()
                    ));
                }
            }
            _ => {}
        }
    }

    /// Commit the current search preview on Enter.
    fn submit_search(&mut self) {
        if let AppMode::Search { preview, .. } = &mut self.mode
            && let Some(preview) = preview.take()
        {
            log::debug!(
                "Submit search: query={:?}, current={:?}",
                preview.query.as_str(),
                preview.current
            );
            self.search = Some(preview);
        }
        self.exit_search_mode();
    }

    /// Jump to next/previous match using the stored search state.
    fn jump_to_next_match(&mut self, reverse: bool) {
        let Some(ref search) = self.search else {
            log::debug!("Jump to next match: no active search");
            return;
        };

        let direction = if reverse {
            search.direction.opposite()
        } else {
            search.direction
        };

        log::debug!(
            "Jump to next match: reverse={reverse}, direction={direction:?}, current={:?}",
            search.current
        );

        let old_current_line = search.current.map(|c| c.line);

        // Compute visible rows, excluding any rows hidden by the section header overlay.
        let visible_rows = self.page.viewport.visible_rows();
        let width = self.page.viewport.width();

        // If the current cursor is outside the visible area, re-anchor it
        // to the first visible match instead of jumping from the old position.
        let needs_reanchor = match search.current {
            Some(c) => !is_match_visible(&mut self.page.doc, &search.query, c, visible_rows, width),
            None => false,
        };

        if needs_reanchor {
            let reanchored = find_first_match_in_viewport(
                &mut self.page.doc,
                &search.query,
                visible_rows,
                width,
            );
            log::debug!("Cursor outside viewport, re-anchor: {reanchored:?}");

            if let Some(pos) = reanchored {
                let mut dirty = Vec::new();
                if let Some(old_line) = old_current_line {
                    dirty.push(old_line);
                }
                dirty.push(pos.line);
                self.redraw = RedrawState::SearchHighlight {
                    old_match_lines: dirty,
                };
                if let Some(ref mut search) = self.search {
                    search.current = Some(pos);
                }
                return;
            }
            // No matches in viewport; fall through to search from viewport top.
        }

        if !needs_reanchor {
            // Try to move to the next match within the same line first.
            if let Some(current) = search.current
                && let Some(next_mi) = search::find_next_match_in_line(
                    &mut self.page.doc,
                    &search.query,
                    current,
                    direction,
                )
            {
                log::debug!("Next match on same line: index={next_mi}");
                if let Some(ref mut search) = self.search {
                    search.current = Some(MatchPosition {
                        line: current.line,
                        match_index: next_mi,
                    });
                }
                // Same line, just current match index changed — delta redraw.
                self.redraw = RedrawState::SearchHighlight {
                    old_match_lines: vec![current.line],
                };
                return;
            }
        }

        // Search the next line from the appropriate starting point.
        let from = if needs_reanchor {
            // Use the first visible row's line (after overlay), not top_line_index().
            visible_rows
                .first()
                .map(|r| r.line_index)
                .unwrap_or_else(|| self.page.viewport.top_line_index())
        } else {
            match search.current {
                Some(pos) => match direction {
                    SearchDirection::Forward => {
                        if pos.line + 1 < self.page.doc.line_count() {
                            pos.line + 1
                        } else {
                            0
                        }
                    }
                    SearchDirection::Backward => {
                        if pos.line > 0 {
                            pos.line - 1
                        } else {
                            self.page.doc.line_count().saturating_sub(1)
                        }
                    }
                },
                None => self.page.viewport.top_line_index(),
            }
        };

        let matched = search::find_next_match(&mut self.page.doc, &search.query, from, direction);
        log::debug!("Next match from line {from}: {matched:?}");
        if let Some(ref pos) = matched {
            let old_rows = self.page.viewport.rows().to_vec();
            let old_header_height = self.page.resolve_header().len();

            let scrolled = self.page.jump_to_visible(pos.line);

            let mut dirty = Vec::new();
            if let Some(old_line) = old_current_line {
                dirty.push(old_line);
            }
            dirty.push(pos.line);

            if scrolled {
                let new_header_height = self.page.resolve_header().len();
                if old_header_height == new_header_height {
                    if let Some((n, dir)) =
                        compute_scroll_overlap(&old_rows, self.page.viewport.rows())
                    {
                        self.redraw = RedrawState::JumpScroll {
                            scroll_rows: n,
                            direction: dir,
                            highlight_dirty_lines: dirty,
                        };
                    } else {
                        self.redraw = RedrawState::Full;
                    }
                } else {
                    self.redraw = RedrawState::Full;
                }
            } else {
                self.redraw = RedrawState::SearchHighlight {
                    old_match_lines: dirty,
                };
            }
        }
        if let Some(ref mut search) = self.search {
            search.current = matched;
        }
    }

    /// Collect line indices of visible rows that have search matches.
    fn collect_visible_match_lines(&mut self) -> Vec<usize> {
        let search = active_search(&self.mode, &self.search);
        let Some(search) = search else {
            return Vec::new();
        };
        let rows = self.page.viewport.rows();
        let mut lines = Vec::new();
        let mut last_line = None;
        for row in rows {
            if last_line == Some(row.line_index) {
                continue;
            }
            last_line = Some(row.line_index);
            if let Some(line) = self.page.doc.line(row.line_index)
                && line.has_match(&search.query)
            {
                lines.push(row.line_index);
            }
        }
        // Also include header rows.
        let header_rows = self.page.resolve_header();
        let mut last_line = None;
        for row in &header_rows {
            if last_line == Some(row.line_index) {
                continue;
            }
            last_line = Some(row.line_index);
            if let Some(line) = self.page.doc.line(row.line_index)
                && line.has_match(&search.query)
            {
                lines.push(row.line_index);
            }
        }
        lines
    }

    fn scroll_immediate(&mut self, rows: isize) -> io::Result<()> {
        self.scroll_physics.stop();
        self.apply_scroll(rows)
    }

    /// Start or add momentum for an animated scroll.
    /// In instant_scroll mode (tests), scrolls immediately instead.
    fn scroll_animated(&mut self, total_rows: isize) -> io::Result<()> {
        if self.instant_scroll {
            self.scroll_physics.stop();
            self.apply_scroll(total_rows)
        } else {
            log::debug!("Scroll animation impulse: rows={total_rows}");
            self.scroll_physics.impulse(total_rows as f64);
            Ok(())
        }
    }

    fn update_animation(&mut self) -> io::Result<()> {
        if !self.scroll_physics.is_active() {
            return Ok(());
        }

        let rows = self
            .scroll_physics
            .tick(FRAME_DURATION_ANIMATING.as_secs_f64());

        self.apply_scroll(rows)
    }

    /// Apply a scroll and handle section header changes.
    fn apply_scroll(&mut self, rows: isize) -> io::Result<()> {
        let old_header_height = self.page.resolve_header().len();

        if let Some(plan) = self.page.plan_scroll(rows) {
            let new_header_height = self.page.resolve_header().len();

            if old_header_height != new_header_height {
                // Header height changed (section change, push-up, or overlay change):
                // need viewport resize + full redraw.
                let (w, h) = self.screen.size()?;
                self.page.resize(w as usize, h as usize);
                self.redraw = RedrawState::Full;
            } else {
                let search = active_search(&self.mode, &self.search);
                render::apply_scroll(&mut self.screen, &plan, &mut self.page, search)?;
            }
        }
        Ok(())
    }
}

/// Resolve which SearchState is active: preview (during search) or committed.
fn active_search<'a>(
    mode: &'a AppMode,
    committed: &'a Option<SearchState>,
) -> Option<&'a SearchState> {
    match mode {
        AppMode::Search { preview, .. } => preview.as_ref(),
        _ => committed.as_ref(),
    }
}

/// Check if a match is on a wrap row that is actually visible on screen.
fn is_match_visible(
    doc: &mut Document,
    query: &Regex,
    pos: MatchPosition,
    visible_rows: &[ScreenRow],
    width: usize,
) -> bool {
    let Some(line) = doc.line(pos.line) else {
        return false;
    };
    let matches = line.find_matches(query);
    let Some(&(start, _)) = matches.get(pos.match_index) else {
        return false;
    };
    let wrap_index = line.wrap_row_for_plain_offset(width, start);
    visible_rows
        .iter()
        .any(|r| r.line_index == pos.line && r.wrap_index == wrap_index)
}

/// Find the first match that falls on a visible wrap row in the viewport.
fn find_first_match_in_viewport(
    doc: &mut Document,
    query: &Regex,
    visible_rows: &[ScreenRow],
    width: usize,
) -> Option<MatchPosition> {
    for row in visible_rows {
        let line = doc.line(row.line_index)?;
        let matches = line.find_matches(query);
        for (mi, &(start, _)) in matches.iter().enumerate() {
            if line.wrap_row_for_plain_offset(width, start) == row.wrap_index {
                return Some(MatchPosition {
                    line: row.line_index,
                    match_index: mi,
                });
            }
        }
    }
    None
}

/// Detect overlap between old and new viewport rows after a jump.
/// Returns the scroll distance and direction if the viewports overlap.
/// We determine the direction by comparing actual rows rather than using SearchDirection,
/// because wraparound can cause the scroll direction to be opposite to the search direction.
fn compute_scroll_overlap(
    old_rows: &[ScreenRow],
    new_rows: &[ScreenRow],
) -> Option<(usize, Direction)> {
    if old_rows.is_empty() || new_rows.is_empty() {
        return None;
    }
    // Scroll down: new viewport starts partway into old viewport.
    let new_first = new_rows[0];
    for (i, row) in old_rows.iter().enumerate().skip(1) {
        if *row == new_first {
            return Some((i, Direction::Down));
        }
    }
    // Scroll up: old viewport starts partway into new viewport.
    let old_first = old_rows[0];
    for (i, row) in new_rows.iter().enumerate().skip(1) {
        if *row == old_first {
            return Some((i, Direction::Up));
        }
    }
    None
}
