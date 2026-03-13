use std::io;
use std::time::{Duration, Instant};

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

use crate::document::Document;
use crate::line_editor::LineEditor;
use crate::screen::{self, Screen, SearchHighlight};

/// Build a SearchHighlight from an optional search state.
fn search_highlight(search: &Option<SearchState>) -> Option<SearchHighlight<'_>> {
    search.as_ref().map(|s| SearchHighlight {
        query: &s.query,
        current_line: s.current_line,
    })
}
use crate::screen_state::ScreenState;
use crate::scroll::ScrollAnimation;
use crate::search::{SearchDirection, SearchState};
use crate::status_line::StatusLine;

const FRAME_DURATION_ANIMATING: Duration = Duration::from_millis(8);
const FRAME_DURATION_IDLE: Duration = Duration::from_millis(50);
const SCROLL_ANIMATION_DURATION: Duration = Duration::from_millis(200);

/// Current input mode of the application.
enum AppMode {
    View,
    Search {
        direction: SearchDirection,
        editor: LineEditor,
    },
}

pub struct App<S> {
    screen: S,
    doc: Document,
    state: ScreenState,
    status: StatusLine,
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
        let state = ScreenState::new(&mut doc, w as usize, h as usize);

        Ok(Self {
            screen,
            doc,
            state,
            status: StatusLine::new(),
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
        let sh = search_highlight(&self.search);
        screen::draw_full_page(
            &mut self.screen,
            &mut self.doc,
            &self.state,
            &self.status,
            sh.as_ref(),
        )?;
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
                        self.state.resize(&mut self.doc, w as usize, h as usize);
                        self.needs_full_redraw = true;
                    }
                    _ => {}
                }
            }

            // 2. Update animation
            self.update_animation()?;

            // 3. Render if needed
            if self.needs_full_redraw {
                let sh = search_highlight(&self.search);
                screen::draw_full_page(
                    &mut self.screen,
                    &mut self.doc,
                    &self.state,
                    &self.status,
                    sh.as_ref(),
                )?;
                self.needs_full_redraw = false;
                self.needs_status_redraw = false;
            } else if self.needs_status_redraw {
                screen::draw_status_line(
                    &mut self.screen,
                    &self.status,
                    self.state.rows().len() as u16,
                )?;
                self.screen.flush()?;
                self.needs_status_redraw = false;
            }
        }
    }

    /// Returns true if the app should quit.
    fn handle_key(&mut self, key: KeyEvent) -> io::Result<bool> {
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
                self.start_scroll_animation(self.state.content_height() as isize / 2);
            }
            KeyCode::Char('u') => {
                self.start_scroll_animation(-(self.state.content_height() as isize / 2));
            }
            KeyCode::Char('f') | KeyCode::Char(' ') => {
                self.start_scroll_animation(self.state.content_height() as isize);
            }
            KeyCode::Char('b') => {
                self.start_scroll_animation(-(self.state.content_height() as isize));
            }
            KeyCode::Char('g') => {
                self.animation = None;
                if self.state.jump_to(&mut self.doc, 0) {
                    self.rendered_offset = 0.0;
                    self.needs_full_redraw = true;
                }
            }
            KeyCode::Char('G') => {
                self.animation = None;
                if self.state.jump_to_end(&mut self.doc) {
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
        self.animation = None;
        self.status.set_content(direction.prompt().to_string());
        self.mode = AppMode::Search {
            direction,
            editor: LineEditor::new(),
        };
        self.needs_status_redraw = true;
    }

    fn exit_search_mode(&mut self) {
        self.mode = AppMode::View;
        self.status.set_content(":".to_string());
        self.needs_status_redraw = true;
    }

    fn handle_key_search(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Enter => {
                self.submit_search();
            }
            KeyCode::Esc => {
                self.exit_search_mode();
            }
            KeyCode::Backspace => {
                if let AppMode::Search {
                    direction, editor, ..
                } = &mut self.mode
                {
                    editor.backspace();
                    self.status
                        .set_content(format!("{}{}", direction.prompt(), editor.input()));
                    self.needs_status_redraw = true;
                }
            }
            KeyCode::Char(ch) => {
                if let AppMode::Search {
                    direction, editor, ..
                } = &mut self.mode
                {
                    editor.insert(ch);
                    self.status
                        .set_content(format!("{}{}", direction.prompt(), editor.input()));
                    self.needs_status_redraw = true;
                }
            }
            _ => {}
        }
    }

    /// Execute search on Enter: build regex, find match, jump to it.
    fn submit_search(&mut self) {
        if let AppMode::Search {
            direction, editor, ..
        } = &self.mode
        {
            let input = editor.input();
            let direction = *direction;

            if !input.is_empty() {
                let re = regex::Regex::new(&regex::escape(&input)).unwrap();
                let from = self.state.rows().first().map(|r| r.line_index).unwrap_or(0);
                let matched = crate::search::find_next_match(&mut self.doc, &re, from, direction);
                if let Some(line_idx) = matched {
                    self.state.jump_to(&mut self.doc, line_idx);
                    self.needs_full_redraw = true;
                }
                self.search = Some(SearchState {
                    query: re,
                    direction,
                    current_line: matched,
                });
            }
        }
        self.exit_search_mode();
    }

    /// Jump to next/previous match using the stored search state.
    fn jump_to_next_match(&mut self, reverse: bool) {
        let Some(ref search) = self.search else {
            return;
        };

        let direction = if reverse {
            search.direction.opposite()
        } else {
            search.direction
        };

        // Start from current match + 1 (or -1 for backward) to avoid re-finding the same line
        let from = match search.current_line {
            Some(line) => match direction {
                SearchDirection::Forward => {
                    if line + 1 < self.doc.line_count() {
                        line + 1
                    } else {
                        0
                    }
                }
                SearchDirection::Backward => {
                    if line > 0 {
                        line - 1
                    } else {
                        self.doc.line_count().saturating_sub(1)
                    }
                }
            },
            None => self.state.rows().first().map(|r| r.line_index).unwrap_or(0),
        };

        let query = search.query.clone();
        let matched = crate::search::find_next_match(&mut self.doc, &query, from, direction);
        if let Some(line_idx) = matched {
            self.state.jump_to(&mut self.doc, line_idx);
            self.needs_full_redraw = true;
        }
        if let Some(ref mut search) = self.search {
            search.current_line = matched;
        }
    }

    fn scroll_immediate(&mut self, rows: isize) -> io::Result<()> {
        // Cancel any running animation
        self.animation = None;

        let plan = if rows > 0 {
            self.state.scroll_down(rows as usize, &mut self.doc)
        } else {
            self.state.scroll_up((-rows) as usize, &mut self.doc)
        };

        if plan.terminal_scroll > 0 {
            let sh = search_highlight(&self.search);
            screen::apply_scroll(
                &mut self.screen,
                &mut self.doc,
                &plan,
                &self.state,
                &self.status,
                sh.as_ref(),
            )?;
            self.rendered_offset += rows as f64;
        }
        Ok(())
    }

    fn start_scroll_animation(&mut self, total_rows: isize) {
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
                self.state.scroll_down(delta as usize, &mut self.doc)
            } else {
                self.state.scroll_up((-delta) as usize, &mut self.doc)
            };

            if plan.terminal_scroll > 0 {
                let sh = search_highlight(&self.search);
                screen::apply_scroll(
                    &mut self.screen,
                    &mut self.doc,
                    &plan,
                    &self.state,
                    &self.status,
                    sh.as_ref(),
                )?;
            }

            self.rendered_offset = current_row as f64;
        }

        if done {
            self.animation = None;
        }

        Ok(())
    }
}
