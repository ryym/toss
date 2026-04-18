mod display;
mod header;
mod mock_screen;
mod scroll;
mod scroll_wrap;
mod search_execution;
mod search_incremental;
mod search_input;
mod search_reanchor;
mod search_with_header;
mod search_wrap;
mod section_header;
mod section_header_multi;

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

use crate::app::App;
use crate::app2::App2;
use crate::document::Document;
use crate::logger;
use crate::options::Options;
use crate::page::Page;
use crate::pager::{Pager, ViewportSize};
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
    let _log_guard = match logger::setup_file_logger() {
        Ok(guard) => guard,
        Err(err) => panic!("failed to setup logger: {}", err),
    };
    let doc = Document::from_string(tc.content.to_string());
    let mut screen = MockScreen::new(tc.screen_width, tc.screen_height);
    screen.set_events(tc.events);

    // let page = Page::new(
    //     doc,
    //     &tc.options,
    //     tc.screen_width as usize,
    //     tc.screen_height as usize,
    // );
    // let mut app = App::new(screen, page).unwrap();

    let size = ViewportSize::new(tc.screen_width as usize, (tc.screen_height - 1) as usize);
    let pager = Pager::new(doc, &size, tc.options);
    let mut app = App2::new(screen, pager).unwrap();

    app.set_instant_scroll();
    app.run().unwrap();
    app.into_screen()
}
