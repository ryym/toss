mod display;
mod header;
mod mock_screen;
mod scroll;
mod scroll_wrap;
mod search_execution;
mod search_incremental;
mod search_input;
mod search_wrap;

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

use crate::app::App;
use crate::document::Document;
use crate::options::Options;
use mock_screen::MockScreen;

pub fn key(ch: char) -> Event {
    Event::Key(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE))
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
    let doc = Document::from_string(tc.content.to_string());
    let mut screen = MockScreen::new(tc.screen_width, tc.screen_height);
    screen.set_events(tc.events);
    let mut app = App::new(screen, doc, tc.options).unwrap();
    app.set_instant_scroll();
    app.run().unwrap();
    app.into_screen()
}
