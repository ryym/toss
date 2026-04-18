// Experimental App implementation using Pager instead of Page.
// All rendering goes through draw_full_page2 (no incremental rendering).

#![allow(unused)]

use std::io;
use std::time::Duration;

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use regex::Regex;

use crate::document::Document;
use crate::line::Row;
use crate::line_editor::LineEditor;
use crate::pager::{Pager, ViewportSize};
use crate::render;
use crate::screen::Screen;
use crate::scroll::ScrollPhysics;
use crate::search::{self, MatchPosition, SearchDirection, SearchState};

const FRAME_DURATION_ANIMATING: Duration = Duration::from_millis(8);
const FRAME_DURATION_IDLE: Duration = Duration::from_millis(50);

enum AppMode {
    View,
    Search {
        direction: SearchDirection,
        editor: LineEditor,
        /// Top content line before search started, for restoring on cancel.
        saved_top_line: usize,
        /// Live search result updated on each keystroke.
        preview: Option<SearchState>,
    },
}

pub struct App2<S> {
    screen: S,
    pager: Pager,
    mode: AppMode,
    search: Option<SearchState>,
    scroll_physics: ScrollPhysics,
    instant_scroll: bool,
    dirty: bool,
}

impl<S: Screen> App2<S> {
    pub fn new(screen: S, pager: Pager) -> io::Result<Self> {
        let (_, h) = screen.size()?;
        let mut scroll_physics = ScrollPhysics::new();
        scroll_physics.configure(h as usize);
        Ok(Self {
            screen,
            pager,
            mode: AppMode::View,
            search: None,
            scroll_physics,
            instant_scroll: false,
            dirty: false,
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
        self.draw()?;
        self.dirty = false;

        loop {
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
                        self.pager
                            .resize(&ViewportSize::new(w as usize, h as usize));
                        self.scroll_physics.configure(h as usize);
                        self.dirty = true;
                    }
                    _ => {}
                }
            }

            self.update_animation();

            if self.dirty {
                self.draw()?;
                self.dirty = false;
            }
        }
    }

    fn draw(&mut self) -> io::Result<()> {
        let status_text = self.status_text();
        let search = active_search(&self.mode, &self.search);
        let (snapshot, doc) = self.pager.snapshot2();
        render::draw_full_page2(&mut self.screen, doc, snapshot, search, &status_text)
    }

    fn status_text(&self) -> String {
        match &self.mode {
            AppMode::View => ":".to_string(),
            AppMode::Search {
                direction, editor, ..
            } => format!("{}{}", direction.prompt(), editor.input_with_cursor()),
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
                self.scroll_immediate(1);
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.scroll_immediate(-1);
            }
            KeyCode::Char('d') => {
                let half = self.pager.content_height() as i32 / 2;
                self.scroll_animated(half);
            }
            KeyCode::Char('u') => {
                let half = -(self.pager.content_height() as i32 / 2);
                self.scroll_animated(half);
            }
            KeyCode::Char('f') | KeyCode::Char(' ') => {
                let full = self.pager.content_height() as i32;
                self.scroll_animated(full);
            }
            KeyCode::Char('b') => {
                let full = -(self.pager.content_height() as i32);
                self.scroll_animated(full);
            }
            KeyCode::Char('g') => {
                self.scroll_physics.stop();
                self.pager.jump_to(0, 0);
                self.dirty = true;
            }
            KeyCode::Char('G') => {
                self.scroll_physics.stop();
                self.pager.jump_to_end();
                self.dirty = true;
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
        let saved_top_line = self.pager.content_top_line_index();
        let editor = LineEditor::new();
        self.mode = AppMode::Search {
            direction,
            editor,
            saved_top_line,
            preview: None,
        };
        self.dirty = true;
    }

    fn exit_search_mode(&mut self) {
        log::debug!("Exit search mode");
        self.mode = AppMode::View;
        self.dirty = true;
    }

    fn cancel_search(&mut self) {
        if let AppMode::Search { saved_top_line, .. } = &self.mode {
            let top = *saved_top_line;
            self.pager.jump_to(top, 0);
        }
        self.exit_search_mode();
    }

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

        if input.is_empty() {
            if let AppMode::Search { preview, .. } = &mut self.mode {
                *preview = None;
            }
            self.pager.jump_to(saved_top_line, 0);
            self.dirty = true;
            return;
        }

        let re = Regex::new(&regex::escape(&input)).unwrap();
        let matched = search::find_next_match(self.pager.doc_mut(), &re, saved_top_line, direction);
        log::debug!("Search preview: query={input:?}, result={matched:?}");

        if let Some(ref pos) = matched {
            self.pager.jump_to(pos.line, 0);
        }

        if let AppMode::Search { preview, .. } = &mut self.mode {
            *preview = Some(SearchState {
                query: re,
                direction,
                current: matched,
            });
        }
        self.dirty = true;
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
                if let AppMode::Search { editor, .. } = &mut self.mode {
                    if editor.input().is_empty() {
                        self.cancel_search();
                        return;
                    }
                    editor.backspace();
                }
                self.update_search_preview();
            }
            KeyCode::Char(ch) => {
                if let AppMode::Search { editor, .. } = &mut self.mode {
                    editor.insert(ch);
                }
                self.update_search_preview();
            }
            KeyCode::Left => {
                if let AppMode::Search { editor, .. } = &mut self.mode {
                    editor.move_left();
                    self.dirty = true;
                }
            }
            KeyCode::Right => {
                if let AppMode::Search { editor, .. } = &mut self.mode {
                    editor.move_right();
                    self.dirty = true;
                }
            }
            _ => {}
        }
    }

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

    fn jump_to_next_match(&mut self, reverse: bool) {
        let Some(search) = self.search.as_ref() else {
            log::debug!("Jump to next match: no active search");
            return;
        };
        let direction = if reverse {
            search.direction.opposite()
        } else {
            search.direction
        };
        // Clone what we need so we can borrow pager mutably below.
        let query = search.query.clone();
        let current = search.current;

        let next = find_next_match_position(&mut self.pager, &query, current, direction);
        if let Some(pos) = next {
            self.pager.jump_to(pos.line, 0);
            if let Some(s) = self.search.as_mut() {
                s.current = Some(pos);
            }
            self.dirty = true;
        }
    }

    fn scroll_immediate(&mut self, rows: i32) {
        self.scroll_physics.stop();
        self.apply_scroll(rows);
    }

    fn scroll_animated(&mut self, total_rows: i32) {
        if self.instant_scroll {
            self.scroll_physics.stop();
            self.apply_scroll(total_rows);
        } else {
            log::debug!("Scroll animation impulse: rows={total_rows}");
            self.scroll_physics.impulse(total_rows as f64);
        }
    }

    fn update_animation(&mut self) {
        if !self.scroll_physics.is_active() {
            return;
        }
        let rows = self
            .scroll_physics
            .tick(FRAME_DURATION_ANIMATING.as_secs_f64());
        self.apply_scroll(rows as i32);
    }

    fn apply_scroll(&mut self, rows: i32) {
        if rows == 0 {
            return;
        }
        let max = self.pager.content_height() as i32;
        let clamped = rows.clamp(-max, max);
        if clamped == 0 {
            return;
        }
        let before = view_signature(&self.pager);
        self.pager.scroll(clamped);
        let after = view_signature(&self.pager);
        if before != after {
            self.dirty = true;
        }
    }
}

/// Lightweight signature of the current rendered view to detect state changes.
/// Captures header sizes and the top content row so scroll-at-boundary (no
/// change) can be distinguished from section-header transitions.
fn view_signature(pager: &Pager) -> (usize, usize, usize, usize) {
    let snapshot = pager.snapshot();
    let (top_line, top_wrap) = snapshot
        .content
        .first()
        .map(|r| (r.line_index, r.wrap_index))
        .unwrap_or((0, 0));
    (
        snapshot.global_header.len(),
        snapshot.section_header.len(),
        top_line,
        top_wrap,
    )
}

fn active_search<'a>(
    mode: &'a AppMode,
    committed: &'a Option<SearchState>,
) -> Option<&'a SearchState> {
    match mode {
        AppMode::Search { preview, .. } => preview.as_ref(),
        _ => committed.as_ref(),
    }
}

/// Find the next match to jump to. Handles re-anchoring when the current match
/// is no longer visible (e.g., after g/G).
fn find_next_match_position(
    pager: &mut Pager,
    query: &Regex,
    current: Option<MatchPosition>,
    direction: SearchDirection,
) -> Option<MatchPosition> {
    let visible_rows = pager.visible_content_rows_cloned();
    if visible_rows.is_empty() {
        return None;
    }

    let needs_reanchor = match current {
        Some(c) => !is_match_visible(pager.doc_mut(), query, c, &visible_rows),
        None => false,
    };

    if needs_reanchor {
        let reanchored = find_first_match_in_viewport(pager.doc_mut(), query, &visible_rows);
        log::debug!("Cursor outside viewport, re-anchor: {reanchored:?}");
        if let Some(pos) = reanchored {
            return Some(pos);
        }
        // Fall through to search from viewport top.
    }

    if !needs_reanchor
        && let Some(current) = current
        && let Some(next_mi) =
            search::find_next_match_in_line(pager.doc_mut(), query, current, direction)
    {
        return Some(MatchPosition {
            line: current.line,
            match_index: next_mi,
        });
    }

    let line_count = pager.doc_mut().line_count();
    let from = match current {
        Some(pos) if !needs_reanchor => match direction {
            SearchDirection::Forward => {
                if pos.line + 1 < line_count {
                    pos.line + 1
                } else {
                    0
                }
            }
            SearchDirection::Backward => {
                if pos.line > 0 {
                    pos.line - 1
                } else {
                    line_count.saturating_sub(1)
                }
            }
        },
        _ => visible_rows[0].line_index,
    };

    search::find_next_match(pager.doc_mut(), query, from, direction)
}

fn is_match_visible(
    doc: &mut Document,
    query: &Regex,
    pos: MatchPosition,
    visible_rows: &[Row],
) -> bool {
    let Some(line) = doc.line(pos.line) else {
        return false;
    };
    let matches = line.find_matches(query);
    let Some(&(start, _)) = matches.get(pos.match_index) else {
        return false;
    };
    let raw_offset = line.plain_to_raw()[start];
    visible_rows
        .iter()
        .any(|r| r.line_index == pos.line && r.raw_range.contains(&raw_offset))
}

fn find_first_match_in_viewport(
    doc: &mut Document,
    query: &Regex,
    visible_rows: &[Row],
) -> Option<MatchPosition> {
    for row in visible_rows {
        let line = doc.line(row.line_index)?;
        let matches = line.find_matches(query);
        for (mi, &(start, _)) in matches.iter().enumerate() {
            let raw_offset = line.plain_to_raw()[start];
            if row.raw_range.contains(&raw_offset) {
                return Some(MatchPosition {
                    line: row.line_index,
                    match_index: mi,
                });
            }
        }
    }
    None
}
