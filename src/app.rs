use std::io;
use std::time::{Duration, Instant};

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

use crate::document::Document;
use crate::screen::{self, Screen};
use crate::screen_state::ScreenState;
use crate::scroll::ScrollAnimation;

const FRAME_DURATION_ANIMATING: Duration = Duration::from_millis(8);
const FRAME_DURATION_IDLE: Duration = Duration::from_millis(50);
const SCROLL_ANIMATION_DURATION: Duration = Duration::from_millis(200);

pub struct App<S> {
    screen: S,
    doc: Document,
    state: ScreenState,
    width: usize,
    height: usize,
    animation: Option<ScrollAnimation>,
    scroll_duration: Duration,
    /// Current scroll position as a float (row offset from top of document).
    /// This is the "rendered" position — the last integer position we drew.
    rendered_offset: f64,
    needs_full_redraw: bool,
}

impl<S: Screen> App<S> {
    pub fn new(screen: S, doc: Document) -> io::Result<Self> {
        let (w, h) = screen.size()?;
        let width = w as usize;
        let height = h as usize;
        let state = ScreenState::new(&doc, width, height);

        Ok(Self {
            screen,
            doc,
            state,
            width,
            height,
            animation: None,
            scroll_duration: SCROLL_ANIMATION_DURATION,
            rendered_offset: 0.0,
            needs_full_redraw: true,
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
        screen::draw_full_page(&mut self.screen, &self.doc, self.state.rows(), self.width)?;
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
                        self.width = w as usize;
                        self.height = h as usize;
                        self.state.resize(&self.doc, self.width, self.height);
                        self.needs_full_redraw = true;
                    }
                    _ => {}
                }
            }

            // 2. Update animation
            self.update_animation()?;

            // 3. Render if needed
            if self.needs_full_redraw {
                screen::draw_full_page(&mut self.screen, &self.doc, self.state.rows(), self.width)?;
                self.needs_full_redraw = false;
            }
        }
    }

    /// Returns true if the app should quit.
    fn handle_key(&mut self, key: KeyEvent) -> io::Result<bool> {
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
                self.start_scroll_animation(self.height as isize / 2);
            }
            KeyCode::Char('u') => {
                self.start_scroll_animation(-(self.height as isize / 2));
            }
            KeyCode::Char('f') | KeyCode::Char(' ') => {
                self.start_scroll_animation(self.height as isize);
            }
            KeyCode::Char('b') => {
                self.start_scroll_animation(-(self.height as isize));
            }
            KeyCode::Char('g') => {
                self.animation = None;
                if self.state.jump_to(&self.doc, 0) {
                    self.rendered_offset = 0.0;
                    self.needs_full_redraw = true;
                }
            }
            KeyCode::Char('G') => {
                self.animation = None;
                if self.state.jump_to_end(&self.doc) {
                    self.needs_full_redraw = true;
                }
            }
            _ => {}
        }
        Ok(false)
    }

    fn scroll_immediate(&mut self, rows: isize) -> io::Result<()> {
        // Cancel any running animation
        self.animation = None;

        let plan = if rows > 0 {
            self.state.scroll_down(rows as usize, &self.doc)
        } else {
            self.state.scroll_up((-rows) as usize, &self.doc)
        };

        if plan.terminal_scroll > 0 {
            screen::apply_scroll(&mut self.screen, &self.doc, &plan, &self.state, self.width)?;
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
                self.state.scroll_down(delta as usize, &self.doc)
            } else {
                self.state.scroll_up((-delta) as usize, &self.doc)
            };

            if plan.terminal_scroll > 0 {
                screen::apply_scroll(&mut self.screen, &self.doc, &plan, &self.state, self.width)?;
            }

            self.rendered_offset = current_row as f64;
        }

        if done {
            self.animation = None;
        }

        Ok(())
    }
}
