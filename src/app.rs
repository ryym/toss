use std::io;
use std::time::{Duration, Instant};

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

use crate::document::Document;
use crate::line_editor::LineEditor;
use crate::page::Page;
use crate::screen::{self, Screen, SearchHighlight};
use crate::scroll::ScrollAnimation;
use crate::search::{self, MatchPosition, SearchDirection, SearchState};
use crate::status_line::StatusLine;
use crate::viewport::Viewport;

const FRAME_DURATION_ANIMATING: Duration = Duration::from_millis(8);
const FRAME_DURATION_IDLE: Duration = Duration::from_millis(50);
const SCROLL_ANIMATION_DURATION: Duration = Duration::from_millis(200);

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
    animation: Option<ScrollAnimation>,
    scroll_duration: Duration,
    /// Current scroll position as a float (row offset from top of document).
    /// This is the "rendered" position — the last integer position we drew.
    rendered_offset: f64,
    needs_full_redraw: bool,
    needs_status_redraw: bool,
}

impl<S: Screen> App<S> {
    pub fn new(screen: S, mut doc: Document) -> io::Result<Self> {
        let (w, h) = screen.size()?;
        let content_height = (h as usize).saturating_sub(1);
        let viewport = Viewport::new(&mut doc, w as usize, content_height);

        Ok(Self {
            screen,
            page: Page {
                doc,
                viewport,
                status: StatusLine::new(),
            },
            mode: AppMode::View,
            search: None,
            animation: None,
            scroll_duration: SCROLL_ANIMATION_DURATION,
            rendered_offset: 0.0,
            needs_full_redraw: true,
            needs_status_redraw: false,
        })
    }

    #[cfg(test)]
    pub fn set_scroll_duration(&mut self, duration: Duration) {
        self.scroll_duration = duration;
    }

    #[cfg(test)]
    pub fn into_screen(self) -> S {
        self.screen
    }

    pub fn run(&mut self) -> io::Result<()> {
        // Initial draw
        let sh = search_highlight(active_search(&self.mode, &self.search));
        screen::draw_full_page(&mut self.screen, &mut self.page, sh.as_ref())?;
        self.needs_full_redraw = false;

        loop {
            // 1. Poll input
            let timeout = if self.animation.is_some() {
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
                        let content_height = (h as usize).saturating_sub(1);
                        self.page
                            .viewport
                            .resize(&mut self.page.doc, w as usize, content_height);
                        self.needs_full_redraw = true;
                    }
                    _ => {}
                }
            }

            // 2. Update animation
            self.update_animation()?;

            // 3. Render if needed
            if self.needs_full_redraw {
                let sh = search_highlight(active_search(&self.mode, &self.search));
                screen::draw_full_page(&mut self.screen, &mut self.page, sh.as_ref())?;
                self.needs_full_redraw = false;
                self.needs_status_redraw = false;
            } else if self.needs_status_redraw {
                screen::draw_status_line(
                    &mut self.screen,
                    &self.page.status,
                    self.page.viewport.rows().len() as u16,
                )?;
                self.screen.flush()?;
                self.needs_status_redraw = false;
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
                self.start_scroll_animation(self.page.viewport.height() as isize / 2);
            }
            KeyCode::Char('u') => {
                self.start_scroll_animation(-(self.page.viewport.height() as isize / 2));
            }
            KeyCode::Char('f') | KeyCode::Char(' ') => {
                self.start_scroll_animation(self.page.viewport.height() as isize);
            }
            KeyCode::Char('b') => {
                self.start_scroll_animation(-(self.page.viewport.height() as isize));
            }
            KeyCode::Char('g') => {
                self.animation = None;
                if self.page.viewport.jump_to(&mut self.page.doc, 0) {
                    self.rendered_offset = 0.0;
                    self.needs_full_redraw = true;
                }
            }
            KeyCode::Char('G') => {
                self.animation = None;
                if self.page.viewport.jump_to_end(&mut self.page.doc) {
                    self.needs_full_redraw = true;
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
        self.animation = None;
        let saved_top_line = self.page.viewport.top_line_index();
        self.page.status.set_content(direction.prompt().to_string());
        self.mode = AppMode::Search {
            direction,
            editor: LineEditor::new(),
            saved_top_line,
            preview: None,
        };
        self.needs_status_redraw = true;
    }

    fn exit_search_mode(&mut self) {
        log::debug!("Exit search mode");
        self.mode = AppMode::View;
        self.page.status.set_content(":".to_string());
        self.needs_status_redraw = true;
    }

    /// Cancel search: discard preview and restore the original scroll position.
    fn cancel_search(&mut self) {
        if let AppMode::Search { saved_top_line, .. } = &self.mode {
            let top = *saved_top_line;
            self.page.viewport.jump_to(&mut self.page.doc, top);
            self.needs_full_redraw = true;
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

        if input.is_empty() {
            // No query: clear preview and restore position.
            if let AppMode::Search { preview, .. } = &mut self.mode {
                *preview = None;
            }
            self.page
                .viewport
                .jump_to(&mut self.page.doc, saved_top_line);
            self.needs_full_redraw = true;
            return;
        }

        let re = regex::Regex::new(&regex::escape(&input)).unwrap();
        let matched = search::find_next_match(&mut self.page.doc, &re, saved_top_line, direction);
        log::debug!("Search preview: query={input:?}, result={matched:?}");

        if let Some(ref pos) = matched {
            self.page.viewport.jump_to(&mut self.page.doc, pos.line);
        }

        if let AppMode::Search { preview, .. } = &mut self.mode {
            *preview = Some(SearchState {
                query: re,
                direction,
                current: matched,
            });
        }
        self.needs_full_redraw = true;
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
                        editor.input()
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
                        editor.input()
                    ));
                }
                self.update_search_preview();
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

        // Try to move to the next match within the same line first.
        if let Some(current) = search.current {
            let query = search.query.clone();
            if let Some(next_mi) =
                search::find_next_match_in_line(&mut self.page.doc, &query, current, direction)
            {
                log::debug!("Next match on same line: index={next_mi}");
                if let Some(ref mut search) = self.search {
                    search.current = Some(MatchPosition {
                        line: current.line,
                        match_index: next_mi,
                    });
                }
                self.needs_full_redraw = true;
                return;
            }
        }

        // No more matches on the current line; search the next line.
        let from = match search.current {
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
        };

        let query = search.query.clone();
        let matched = search::find_next_match(&mut self.page.doc, &query, from, direction);
        log::debug!("Next match from line {from}: {matched:?}");
        if let Some(ref pos) = matched {
            self.page.viewport.jump_to(&mut self.page.doc, pos.line);
            self.needs_full_redraw = true;
        }
        if let Some(ref mut search) = self.search {
            search.current = matched;
        }
    }

    fn scroll_immediate(&mut self, rows: isize) -> io::Result<()> {
        // Cancel any running animation
        self.animation = None;

        let plan = if rows > 0 {
            self.page
                .viewport
                .scroll_down(rows as usize, &mut self.page.doc)
        } else {
            self.page
                .viewport
                .scroll_up((-rows) as usize, &mut self.page.doc)
        };

        if plan.terminal_scroll > 0 {
            let sh = search_highlight(active_search(&self.mode, &self.search));
            screen::apply_scroll(&mut self.screen, &plan, &mut self.page, sh.as_ref())?;
            self.rendered_offset += rows as f64;
        }
        Ok(())
    }

    fn start_scroll_animation(&mut self, total_rows: isize) {
        log::debug!("Start scroll animation: rows={total_rows}");
        let start = self.rendered_offset;
        let target = start + total_rows as f64;
        self.animation = Some(ScrollAnimation::new(start, target, self.scroll_duration));
    }

    fn update_animation(&mut self) -> io::Result<()> {
        let Some(ref anim) = self.animation else {
            return Ok(());
        };

        let now = Instant::now();
        let done = anim.is_done(now);
        let current = if done {
            anim.target()
        } else {
            anim.current_offset(now)
        };

        // How many rows to scroll since last render
        let current_row = current.floor() as isize;
        let rendered_row = self.rendered_offset.floor() as isize;
        let delta = current_row - rendered_row;

        if delta != 0 {
            let plan = if delta > 0 {
                self.page
                    .viewport
                    .scroll_down(delta as usize, &mut self.page.doc)
            } else {
                self.page
                    .viewport
                    .scroll_up((-delta) as usize, &mut self.page.doc)
            };

            if plan.terminal_scroll > 0 {
                let sh = search_highlight(active_search(&self.mode, &self.search));
                screen::apply_scroll(&mut self.screen, &plan, &mut self.page, sh.as_ref())?;
            }

            self.rendered_offset = current_row as f64;
        }

        if done {
            self.animation = None;
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

/// Build a SearchHighlight from an optional search state.
fn search_highlight(search: Option<&SearchState>) -> Option<SearchHighlight<'_>> {
    search.map(|s| SearchHighlight {
        query: &s.query,
        current: s.current,
    })
}
