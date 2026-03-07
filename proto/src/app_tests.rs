use std::time::Duration;

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use pretty_assertions::assert_eq;

use crate::app::App;
use crate::document::Document;
use crate::mock_screen::MockScreen;

fn key(ch: char) -> Event {
    Event::Key(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE))
}

fn run_test(content: &str, width: u16, height: u16, events: Vec<Event>) -> String {
    let doc = Document::from_string(content.to_string());
    let mut screen = MockScreen::new(width, height);
    screen.set_events(events);
    let mut app = App::new(screen, doc).unwrap();
    // Disable animation for deterministic tests
    app.set_scroll_duration(Duration::ZERO);
    app.run().unwrap();
    app.into_screen().out().to_string()
}

#[test]
fn open_and_quit() {
    let out = run_test(
        "line 1\nline 2\nline 3\nline 4\nline 5",
        10,
        3,
        vec![key('q')],
    );
    assert_eq!(
        out,
        "\
line 1
line 2
line 3
-----
[EVENT]:char:q
"
    );
}

#[test]
fn navigate_up_down() {
    let out = run_test(
        "line 1\nline 2\nline 3\nline 4\nline 5",
        10,
        3,
        vec![key('j'), key('j'), key('k'), key('q')],
    );
    assert_eq!(
        out,
        "\
line 1
line 2
line 3
-----
[EVENT]:char:j
line 2
line 3
line 4
-----
[EVENT]:char:j
line 3
line 4
line 5
-----
[EVENT]:char:k
line 2
line 3
line 4
-----
[EVENT]:char:q
"
    );
}

#[test]
fn navigate_top_bottom() {
    let out = run_test(
        "line 1\nline 2\nline 3\nline 4\nline 5\nline 6\nline 7\nline 8",
        10,
        3,
        vec![key('G'), key('g'), key('q')],
    );
    assert_eq!(
        out,
        "\
line 1
line 2
line 3
-----
[EVENT]:char:G
line 6
line 7
line 8
-----
[EVENT]:char:g
line 1
line 2
line 3
-----
[EVENT]:char:q
"
    );
}

#[test]
fn cannot_scroll_past_boundaries() {
    let out = run_test(
        "line 1\nline 2\nline 3",
        10,
        3,
        vec![key('k'), key('j'), key('q')],
    );
    // k at top: no change, no snapshot. j at bottom: no change, no snapshot.
    assert_eq!(
        out,
        "\
line 1
line 2
line 3
-----
[EVENT]:char:k
[EVENT]:char:j
[EVENT]:char:q
"
    );
}

#[test]
fn smooth_scroll_half_page() {
    let out = run_test(
        "line 1\nline 2\nline 3\nline 4\nline 5\nline 6\nline 7\nline 8",
        10,
        4,
        vec![key('d'), key('u'), key('q')],
    );
    // With duration=0, animation completes instantly in one frame.
    // d scrolls down height/2 = 2 rows, u scrolls up 2 rows.
    assert_eq!(
        out,
        "\
line 1
line 2
line 3
line 4
-----
[EVENT]:char:d
line 3
line 4
line 5
line 6
-----
[EVENT]:char:u
line 1
line 2
line 3
line 4
-----
[EVENT]:char:q
"
    );
}

#[test]
fn scroll_with_wrapping() {
    // "abcdefgh" wraps to 2 rows at width 5: "abcde" + "fgh"
    // Total screen rows: short(1) + abcde(1) + fgh(1) + end(1) = 4
    // Screen height is 3, so 1 row of scroll room.
    let out = run_test(
        "short\nabcdefgh\nend",
        5,
        3,
        vec![key('j'), key('j'), key('k'), key('q')],
    );
    assert_eq!(
        out,
        "\
short
abcde
fgh
-----
[EVENT]:char:j
abcde
fgh
end
-----
[EVENT]:char:j
[EVENT]:char:k
short
abcde
fgh
-----
[EVENT]:char:q
"
    );
}
