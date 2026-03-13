mod display_ansi_escape_sequences;
mod open_and_quit;
mod scroll_cannot_past_boundaries;
mod scroll_down_reveals_wrap_continuation;
mod scroll_half_page;
mod scroll_top_bottom;
mod scroll_top_bottom_with_wrap;
mod scroll_up_down;
mod scroll_up_reveals_wrap_start;
mod scroll_with_soft_wrap;
mod wrap_midline_at_top_of_screen;

use std::time::Duration;

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

use crate::app::App;
use crate::document::Document;
use crate::mock_screen::MockScreen;

pub fn key(ch: char) -> Event {
    Event::Key(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE))
}

pub struct TestCase {
    pub content: &'static str,
    pub screen_width: u16,
    pub screen_height: u16,
    pub events: Vec<Event>,
}

pub fn run_test(tc: TestCase) -> String {
    let doc = Document::from_string(tc.content.to_string());
    let mut screen = MockScreen::new(tc.screen_width, tc.screen_height);
    screen.set_events(tc.events);
    let mut app = App::new(screen, doc).unwrap();
    app.set_scroll_duration(Duration::ZERO);
    app.run().unwrap();
    app.into_screen().out().to_string()
}
