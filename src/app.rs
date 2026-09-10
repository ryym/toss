use std::io;
use std::time::Duration;

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

use crate::line_editor::LineEdit;
use crate::pager::{Pager, PagerMode};
use crate::renderer::Renderer;
use crate::screen::Screen;
use crate::scroll::ScrollPhysics;
use crate::search::SearchDirection;

/// Poll cadence while a scroll animation is running. It is also the fixed time step the
/// animation advances by, so the motion does not depend on how long a frame really took.
const FRAME_DURATION_ANIMATING: Duration = Duration::from_millis(8);
/// Poll cadence when nothing is in flight, chosen to keep an idle pager cheap.
const FRAME_DURATION_IDLE: Duration = Duration::from_millis(50);
/// Poll cadence while input is still streaming in, so new lines surface promptly
/// without the busy cost of the animation cadence.
const FRAME_DURATION_LOADING: Duration = Duration::from_millis(16);

/// Result of handling a terminal event or key input.
/// `Continue` carries whether the page state changed and so needs a render.
enum AppAction {
    Continue(bool),
    Quit,
}

/// Drives the pager: reads terminal events, turns them into page operations, and renders
/// the result.
///
/// [`App`] owns the event loop. It polls for input at a cadence that follows what is in
/// flight (a running scroll animation, streaming input, or neither), asks [`Pager`] to
/// update the page state, and hands the resulting page to [`Renderer`]. Scrolling by a
/// page or half a page is animated by [`ScrollPhysics`], unless
/// [`Self::set_instant_scroll`] turned the animation off.
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

    /// Make animated scrolls land immediately instead of easing over several frames.
    /// Tests need the page to settle within the call that scrolled it.
    pub fn set_instant_scroll(&mut self) {
        self.instant_scroll = true;
    }

    pub fn into_screen(self) -> S {
        self.renderer.into_screen()
    }

    pub fn doc(&self) -> &crate::document::Document {
        self.pager.doc()
    }

    /// Run the event loop until the user quits, rendering whenever the page changed.
    pub fn run(&mut self) -> io::Result<()> {
        self.pager.pump_input();
        self.render()?;

        loop {
            let event_changed = match self.handle_terminal_event()? {
                AppAction::Quit => return Ok(()),
                AppAction::Continue(changed) => changed,
            };
            let input_changed = self.pager.pump_input();
            let anim_changed = self.update_scroll_animation();

            if event_changed || input_changed || anim_changed {
                self.render()?;
            }
        }
    }

    fn render(&mut self) -> io::Result<()> {
        let (snapshot, doc) = self.pager.snapshot();
        self.renderer.render(doc, snapshot)
    }

    /// Wait for the next terminal event, up to the poll cadence for the current state,
    /// and apply it.
    fn handle_terminal_event(&mut self) -> io::Result<AppAction> {
        let timeout = if self.scroll_physics.is_active() {
            FRAME_DURATION_ANIMATING
        } else if self.pager.is_loading() {
            FRAME_DURATION_LOADING
        } else {
            FRAME_DURATION_IDLE
        };
        let Some(event) = self.renderer.poll_event(timeout)? else {
            return Ok(AppAction::Continue(false));
        };
        match event {
            Event::Key(key) => Ok(self.handle_key(key)),
            Event::Resize(w, h) => {
                log::debug!("Resize: {w}x{h}");
                let changed = self.pager.resize(usize::from(w), usize::from(h));
                self.scroll_physics.configure(usize::from(h));
                Ok(AppAction::Continue(changed))
            }
            _ => Ok(AppAction::Continue(false)),
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
        let changed = match key.code {
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
                self.pager.jump_to(0)
            }
            KeyCode::Char('G') => {
                self.scroll_physics.stop();
                self.pager.jump_to_end()
            }
            KeyCode::Char('/') => self.pager.start_search_input(SearchDirection::Forward),
            KeyCode::Char('?') => self.pager.start_search_input(SearchDirection::Backward),
            KeyCode::Char('n') => {
                self.scroll_physics.stop();
                self.pager.jump_to_next_match(false)
            }
            KeyCode::Char('N') => {
                self.scroll_physics.stop();
                self.pager.jump_to_next_match(true)
            }
            _ => false,
        };
        AppAction::Continue(changed)
    }

    fn handle_key_search(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Enter => self.pager.submit_search(),
            KeyCode::Esc => self.pager.cancel_search_input(),
            KeyCode::Backspace => {
                if self.pager.has_search_input() {
                    self.pager
                        .update_search_query(LineEdit::DeleteCharBeforeCursor)
                } else {
                    self.pager.cancel_search_input()
                }
            }
            KeyCode::Char(ch) => self.pager.update_search_query(LineEdit::AddChar(ch)),
            KeyCode::Left => self.pager.update_search_query(LineEdit::MoveCursorLeft),
            KeyCode::Right => self.pager.update_search_query(LineEdit::MoveCursorRight),
            _ => false,
        }
    }

    /// Scroll by `rows` at once, cancelling any animation in flight.
    fn scroll_immediate(&mut self, rows: i32) -> bool {
        self.scroll_physics.stop();
        self.apply_scroll(rows)
    }

    /// Start or add momentum for a scroll animated over the following frames.
    /// Under [`Self::set_instant_scroll`] the whole distance is applied at once instead.
    fn scroll_animated(&mut self, total_rows: i32) -> bool {
        if self.instant_scroll {
            self.scroll_physics.stop();
            self.apply_scroll(total_rows)
        } else {
            log::debug!("Scroll animation impulse: rows={total_rows}");
            self.scroll_physics.impulse(f64::from(total_rows));
            false
        }
    }

    /// Advance a running scroll animation by one frame. Returns whether the page moved.
    fn update_scroll_animation(&mut self) -> bool {
        if !self.scroll_physics.is_active() {
            return false;
        }
        let rows = self
            .scroll_physics
            .tick(FRAME_DURATION_ANIMATING.as_secs_f64());
        self.apply_scroll(rows as i32)
    }

    /// Scroll the page by `rows`, never further than one screenful of content in one step.
    /// Returns whether the page actually moved.
    fn apply_scroll(&mut self, rows: i32) -> bool {
        if rows == 0 {
            return false;
        }
        let max = self.pager.content_height() as i32;
        let clamped = rows.clamp(-max, max);
        if clamped == 0 {
            return false;
        }
        self.pager.scroll(clamped)
    }
}
