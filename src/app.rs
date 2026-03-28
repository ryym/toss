use std::io;
use std::time::Duration;

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

use crate::line_editor::LineEditor;
use crate::page::Page;
use crate::screen::{self, Screen};
use crate::scroll::ScrollPhysics;
use crate::search::{self, MatchPosition, SearchDirection, SearchState};

const FRAME_DURATION_ANIMATING: Duration = Duration::from_millis(8);
const FRAME_DURATION_IDLE: Duration = Duration::from_millis(50);

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
    needs_full_redraw: bool,
    needs_status_redraw: bool,
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
            needs_full_redraw: true,
            needs_status_redraw: false,
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
        screen::draw_full_page(&mut self.screen, &mut self.page, search)?;
        self.needs_full_redraw = false;

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
                        self.needs_full_redraw = true;
                    }
                    _ => {}
                }
            }

            // 2. Update animation
            self.update_animation()?;

            // 3. Render if needed
            if self.needs_full_redraw {
                let (_, h) = self.screen.size()?;
                self.page.sync_section_for_redraw(h as usize);
                let search = active_search(&self.mode, &self.search);
                screen::draw_full_page(&mut self.screen, &mut self.page, search)?;
                self.needs_full_redraw = false;
                self.needs_status_redraw = false;
            } else if self.needs_status_redraw {
                screen::draw_status_line(&mut self.screen, &mut self.page)?;
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
                    self.needs_full_redraw = true;
                }
            }
            KeyCode::Char('G') => {
                self.scroll_physics.stop();
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
            self.page.jump_to_visible(pos.line);
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
                self.needs_status_redraw = true;
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
                self.needs_status_redraw = true;
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

        let matched = search::find_next_match(&mut self.page.doc, &search.query, from, direction);
        log::debug!("Next match from line {from}: {matched:?}");
        if let Some(ref pos) = matched {
            self.page.jump_to_visible(pos.line);
            self.needs_full_redraw = true;
        }
        if let Some(ref mut search) = self.search {
            search.current = matched;
        }
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

        if self.page.plan_scroll(rows) {
            let new_header_height = self.page.resolve_header().len();

            if old_header_height != new_header_height {
                // Header height changed (section change, push-up, or overlay change):
                // need viewport resize + full redraw.
                let (w, h) = self.screen.size()?;
                self.page.resize(w as usize, h as usize);
            }
            // Always use full redraw since we no longer use incremental terminal scroll.
            self.needs_full_redraw = true;
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
