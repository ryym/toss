use std::io;
use std::time::Duration;

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

use crate::line_editor::LineEdit;
use crate::pager::{Pager, PagerMode};
use crate::renderer::Renderer;
use crate::screen::Screen;
use crate::scroll::ScrollPhysics;
use crate::search::SearchDirection;

const FRAME_DURATION_ANIMATING: Duration = Duration::from_millis(8);
const FRAME_DURATION_IDLE: Duration = Duration::from_millis(50);

pub struct App<S: Screen> {
    renderer: Renderer<S>,
    pager: Pager,
    scroll_physics: ScrollPhysics,
    instant_scroll: bool,
    dirty: bool,
}

impl<S: Screen> App<S> {
    pub fn new(screen: S, pager: Pager) -> io::Result<Self> {
        let size = screen.size()?;
        let renderer = Renderer::new(screen);
        let mut scroll_physics = ScrollPhysics::new();
        scroll_physics.configure(size.height());
        Ok(Self {
            renderer,
            pager,
            scroll_physics,
            instant_scroll: false,
            dirty: false,
        })
    }

    pub fn set_instant_scroll(&mut self) {
        self.instant_scroll = true;
    }

    pub fn into_screen(self) -> S {
        self.renderer.into_screen()
    }

    pub fn run(&mut self) -> io::Result<()> {
        self.render()?;
        self.dirty = false;

        loop {
            let timeout = if self.scroll_physics.is_active() {
                FRAME_DURATION_ANIMATING
            } else {
                FRAME_DURATION_IDLE
            };

            if let Some(event) = self.renderer.poll_event(timeout)? {
                match event {
                    Event::Key(key) => {
                        if self.handle_key(key)? {
                            return Ok(());
                        }
                    }
                    Event::Resize(w, h) => {
                        log::debug!("Resize: {w}x{h}");
                        self.pager.resize(w as usize, h as usize);
                        self.scroll_physics.configure(h as usize);
                        self.dirty = true;
                    }
                    _ => {}
                }
            }

            self.update_animation();

            if self.dirty {
                self.render()?;
                self.dirty = false;
            }
        }
    }

    fn render(&mut self) -> io::Result<()> {
        let status_text = self.pager.status_text();
        let (snapshot, doc) = self.pager.snapshot();
        self.renderer.render(doc, snapshot, &status_text)
    }

    /// Returns true if the app should quit.
    fn handle_key(&mut self, key: KeyEvent) -> io::Result<bool> {
        log::debug!("Key: {key:?}");
        match self.pager.mode() {
            PagerMode::View => self.handle_key_view(key),
            PagerMode::SearchInput(_) => {
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
                self.pager.jump_to(0);
                self.dirty = true;
            }
            KeyCode::Char('G') => {
                self.scroll_physics.stop();
                self.pager.jump_to_end();
                self.dirty = true;
            }
            KeyCode::Char('/') => {
                self.pager.start_search_input(SearchDirection::Forward);
                self.dirty = true;
            }
            KeyCode::Char('?') => {
                self.pager.start_search_input(SearchDirection::Backward);
                self.dirty = true;
            }
            KeyCode::Char('n') => {
                self.dirty = self.pager.jump_to_next_match(false);
            }
            KeyCode::Char('N') => {
                self.dirty = self.pager.jump_to_next_match(true);
            }
            _ => {}
        }
        Ok(false)
    }

    fn handle_key_search(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Enter => {
                self.pager.submit_search();
                self.dirty = true;
            }
            KeyCode::Esc => {
                self.pager.cancel_search_input();
                self.dirty = true;
            }
            KeyCode::Backspace => {
                if self.pager.has_search_input() {
                    self.pager
                        .update_search_query(LineEdit::DeleteCharBeforeCursor);
                } else {
                    self.pager.cancel_search_input();
                }
                self.dirty = true;
            }
            KeyCode::Char(ch) => {
                self.pager.update_search_query(LineEdit::AddChar(ch));
                self.dirty = true;
            }
            KeyCode::Left => {
                self.pager.update_search_query(LineEdit::MoveCursorLeft);
                self.dirty = true;
            }
            KeyCode::Right => {
                self.pager.update_search_query(LineEdit::MoveCursorRight);
                self.dirty = true;
            }
            _ => {}
        }
    }

    fn scroll_immediate(&mut self, rows: i32) {
        self.scroll_physics.stop();
        self.apply_scroll(rows);
    }

    /// Start or add momentum for an animated scroll.
    /// In instant_scroll mode (tests), scrolls immediately instead.
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
        let scroll_rows = self.pager.scroll(clamped);
        self.dirty = scroll_rows != 0;
    }
}
