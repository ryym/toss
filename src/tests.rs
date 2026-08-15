// See README.md in this directory for the MockScreen-based test approach.
mod display;
mod header;
mod heading;
mod heading_multi;
mod mock_screen;
mod resize;
mod scroll;
mod scroll_wrap;
mod search_execution;
mod search_incremental;
mod search_input;
mod search_reanchor;
mod search_regex;
mod search_with_header;
mod search_wrap;
mod streaming;

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

use crate::app::App;
use crate::document::Document;
use crate::logger;
use crate::options::Options;
use crate::pager::Pager;
use crate::screen::ScreenSize;
use mock_screen::MockScreen;

pub fn key(ch: char) -> Event {
    Event::Key(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE))
}

pub fn esc() -> Event {
    Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))
}

pub fn enter() -> Event {
    Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
}

pub fn backspace() -> Event {
    Event::Key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE))
}

pub fn resize(width: u16, height: u16) -> Event {
    Event::Resize(width, height)
}

pub struct TestCase {
    pub content: &'static str,
    pub screen_width: u16,
    pub screen_height: u16,
    pub events: Vec<Event>,
    pub options: Options,
}

impl Default for TestCase {
    fn default() -> Self {
        Self {
            content: "",
            screen_width: 80,
            screen_height: 24,
            events: vec![],
            options: Options::default(),
        }
    }
}

pub fn run_test_screen(tc: TestCase) -> MockScreen {
    let _log_guard = match logger::setup_file_logger() {
        Ok(guard) => guard,
        Err(err) => panic!("failed to setup logger: {}", err),
    };
    let doc = Document::from_string(tc.content.to_string());
    let pager = Pager::new(
        doc,
        tc.options,
        ScreenSize::new(tc.screen_width, tc.screen_height),
    );
    let mut screen = MockScreen::new(tc.screen_width, tc.screen_height);
    screen.set_events(tc.events);
    let mut app = App::new(screen, pager).unwrap();
    app.set_instant_scroll();
    app.run().unwrap();
    app.into_screen()
}
