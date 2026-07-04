use std::io;
use std::time::Duration;

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

use crate::line_editor::LineEdit;
use crate::pager::{PageUpdate, Pager, PagerMode};
use crate::renderer::Renderer;
use crate::screen::Screen;
use crate::scroll::ScrollPhysics;
use crate::search::SearchDirection;

const FRAME_DURATION_ANIMATING: Duration = Duration::from_millis(8);
const FRAME_DURATION_IDLE: Duration = Duration::from_millis(50);
/// Poll cadence while input is still streaming in, so new lines surface promptly
/// without the busy cost of the animation cadence.
const FRAME_DURATION_LOADING: Duration = Duration::from_millis(16);

/// Result of handling a terminal event or key input.
enum AppAction {
    Continue(Option<PageUpdate>),
    Quit,
}

pub struct App<S: Screen> {
    renderer: Renderer<S>,
    pager: Pager,
    scroll_physics: ScrollPhysics,
    instant_scroll: bool,
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
        })
    }

    pub fn set_instant_scroll(&mut self) {
        self.instant_scroll = true;
    }

    pub fn into_screen(self) -> S {
        self.renderer.into_screen()
    }

    pub fn doc(&self) -> &crate::document::Document {
        self.pager.doc()
    }

    pub fn run(&mut self) -> io::Result<()> {
        self.pager.pump_input();
        self.render(PageUpdate::Full)?;

        loop {
            let event_update = match self.handle_terminal_event()? {
                AppAction::Quit => return Ok(()),
                AppAction::Continue(update) => update,
            };
            let input_update = self.pager.pump_input();
            let scroll_anim_update = self.update_scroll_animation();

            let update = [scroll_anim_update, event_update, input_update]
                .into_iter()
                .flatten()
                .reduce(|a, b| a.combine(b));
            if let Some(update) = update {
                self.render(update)?;
            }
        }
    }

    fn render(&mut self, update: PageUpdate) -> io::Result<()> {
        let (snapshot, doc) = self.pager.snapshot();
        self.renderer.render(doc, snapshot, update)
    }

    fn handle_terminal_event(&mut self) -> io::Result<AppAction> {
        let timeout = if self.scroll_physics.is_active() {
            FRAME_DURATION_ANIMATING
        } else if self.pager.is_loading() {
            FRAME_DURATION_LOADING
        } else {
            FRAME_DURATION_IDLE
        };
        let Some(event) = self.renderer.poll_event(timeout)? else {
            return Ok(AppAction::Continue(None));
        };
        match event {
            Event::Key(key) => Ok(self.handle_key(key)),
            Event::Resize(w, h) => {
                log::debug!("Resize: {w}x{h}");
                let update = self.pager.resize(usize::from(w), usize::from(h));
                self.scroll_physics.configure(usize::from(h));
                Ok(AppAction::Continue(Some(update)))
            }
            _ => Ok(AppAction::Continue(None)),
        }
    }

    fn handle_key(&mut self, key: KeyEvent) -> AppAction {
        log::debug!("Key: {key:?}");
        match self.pager.mode() {
            PagerMode::View => self.handle_key_view(key),
            PagerMode::SearchInput(_) => AppAction::Continue(self.handle_key_search(key)),
        }
    }

    fn handle_key_view(&mut self, key: KeyEvent) -> AppAction {
        let update = match key.code {
            KeyCode::Char('q') | KeyCode::Esc => return AppAction::Quit,
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                return AppAction::Quit;
            }
            KeyCode::Char('j') | KeyCode::Down => self.scroll_immediate(1),
            KeyCode::Char('k') | KeyCode::Up => self.scroll_immediate(-1),
            KeyCode::Char('d') => {
                let half = self.pager.content_height() as i32 / 2;
                self.scroll_animated(half)
            }
            KeyCode::Char('u') => {
                let half = -(self.pager.content_height() as i32 / 2);
                self.scroll_animated(half)
            }
            KeyCode::Char('f') | KeyCode::Char(' ') => {
                let full = self.pager.content_height() as i32;
                self.scroll_animated(full)
            }
            KeyCode::Char('b') => {
                let full = -(self.pager.content_height() as i32);
                self.scroll_animated(full)
            }
            KeyCode::Char('g') => {
                self.scroll_physics.stop();
                Some(self.pager.jump_to(0))
            }
            KeyCode::Char('G') => {
                self.scroll_physics.stop();
                Some(self.pager.jump_to_end())
            }
            KeyCode::Char('/') => Some(self.pager.start_search_input(SearchDirection::Forward)),
            KeyCode::Char('?') => Some(self.pager.start_search_input(SearchDirection::Backward)),
            KeyCode::Char('n') => {
                self.scroll_physics.stop();
                self.pager.jump_to_next_match(false)
            }
            KeyCode::Char('N') => {
                self.scroll_physics.stop();
                self.pager.jump_to_next_match(true)
            }
            _ => None,
        };
        AppAction::Continue(update)
    }

    fn handle_key_search(&mut self, key: KeyEvent) -> Option<PageUpdate> {
        match key.code {
            KeyCode::Enter => Some(self.pager.submit_search()),
            KeyCode::Esc => Some(self.pager.cancel_search_input()),
            KeyCode::Backspace => {
                let update = if self.pager.has_search_input() {
                    self.pager
                        .update_search_query(LineEdit::DeleteCharBeforeCursor)
                } else {
                    self.pager.cancel_search_input()
                };
                Some(update)
            }
            KeyCode::Char(ch) => Some(self.pager.update_search_query(LineEdit::AddChar(ch))),
            KeyCode::Left => Some(self.pager.update_search_query(LineEdit::MoveCursorLeft)),
            KeyCode::Right => Some(self.pager.update_search_query(LineEdit::MoveCursorRight)),
            _ => None,
        }
    }

    fn scroll_immediate(&mut self, rows: i32) -> Option<PageUpdate> {
        self.scroll_physics.stop();
        self.apply_scroll(rows)
    }

    /// Start or add momentum for an animated scroll.
    /// In instant_scroll mode (tests), scrolls immediately instead.
    fn scroll_animated(&mut self, total_rows: i32) -> Option<PageUpdate> {
        if self.instant_scroll {
            self.scroll_physics.stop();
            self.apply_scroll(total_rows)
        } else {
            log::debug!("Scroll animation impulse: rows={total_rows}");
            self.scroll_physics.impulse(f64::from(total_rows));
            None
        }
    }

    fn update_scroll_animation(&mut self) -> Option<PageUpdate> {
        if !self.scroll_physics.is_active() {
            return None;
        }
        let rows = self
            .scroll_physics
            .tick(FRAME_DURATION_ANIMATING.as_secs_f64());
        self.apply_scroll(rows as i32)
    }

    fn apply_scroll(&mut self, rows: i32) -> Option<PageUpdate> {
        if rows == 0 {
            return None;
        }
        let max = self.pager.content_height() as i32;
        let clamped = rows.clamp(-max, max);
        if clamped == 0 {
            return None;
        }
        self.pager.scroll(clamped)
    }
}
