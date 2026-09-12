// See README.md in this directory for the MockScreen-based test approach.
mod display;
mod header;
mod heading;
mod heading_multi;
mod heading_resize;
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

use std::ffi::OsString;
use std::io;

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

use crate::run::{RunConfig, run_with};
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

#[non_exhaustive]
pub struct TestCase {
    pub args: Vec<&'static str>,
    pub content: &'static str,
    pub screen_width: u16,
    pub screen_height: u16,
    pub events: Vec<Event>,
}

impl Default for TestCase {
    fn default() -> Self {
        Self {
            content: "",
            screen_width: 80,
            screen_height: 24,
            events: vec![],
            args: vec![],
        }
    }
}

#[derive(Debug)]
pub struct TestResult {
    output: String,
}

impl TestResult {
    /// The output log: consumed events and grid snapshots, in order.
    /// See src/tests/README.md for the format.
    pub fn output(&self) -> &str {
        &self.output
    }
}

/// Run a test case using [`MockScreen`] through the real entry point and return its output.
/// The document is piped in as stdin would be, so it takes the same route
/// through `run_with` as `command | toss` does.
pub fn run_test(tc: TestCase) -> TestResult {
    let mut args: Vec<OsString> = vec!["toss".into()];
    args.extend(tc.args.iter().map(OsString::from));

    let size = ScreenSize::new(tc.screen_width, tc.screen_height);
    let events = tc.events;

    let mut buf: Vec<u8> = Vec::new();
    if let Err(err) = run_with(RunConfig {
        args,
        terminal_size: size,
        shell_lines: 1,
        instant_scroll: true,
        stdin: io::Cursor::new(tc.content.as_bytes().to_vec()),
        stdin_is_terminal: false,
        stdout: &mut buf,
        make_screen: |w| {
            let mut screen = MockScreen::new(w, size);
            screen.set_events(events);
            Ok(screen)
        },
        // Settle the document before rendering to keep the first frame deterministic.
        wait_for_all_input: true,
    }) {
        panic!("run failed: {err}");
    }

    TestResult {
        output: output_to_string(&buf),
    }
}

fn output_to_string(buf: &[u8]) -> String {
    String::from_utf8_lossy(buf).into_owned()
}
