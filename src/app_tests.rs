use std::time::Duration;

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use pretty_assertions::assert_eq;

use crate::app::App;
use crate::document::Document;
use crate::mock_screen::MockScreen;

fn key(ch: char) -> Event {
    Event::Key(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE))
}

struct TestCase {
    content: &'static str,
    screen_width: u16,
    screen_height: u16,
    events: Vec<Event>,
}

fn run_test(tc: TestCase) -> String {
    let doc = Document::from_string(tc.content.to_string());
    let mut screen = MockScreen::new(tc.screen_width, tc.screen_height);
    screen.set_events(tc.events);
    let mut app = App::new(screen, doc).unwrap();
    app.set_scroll_duration(Duration::ZERO);
    app.run().unwrap();
    app.into_screen().out().to_string()
}

#[test]
fn open_and_quit() {
    let out = run_test(TestCase {
        screen_width: 10,
        screen_height: 3,
        content: "line 1\nline 2\nline 3\nline 4\nline 5",
        events: vec![key('q')],
    });
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
    let out = run_test(TestCase {
        screen_width: 10,
        screen_height: 3,
        content: "line 1\nline 2\nline 3\nline 4\nline 5",
        events: vec![key('j'), key('j'), key('k'), key('q')],
    });
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
    let out = run_test(TestCase {
        screen_width: 10,
        screen_height: 3,
        content: "line 1\nline 2\nline 3\nline 4\nline 5\nline 6\nline 7\nline 8",
        events: vec![key('G'), key('g'), key('q')],
    });
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
    let out = run_test(TestCase {
        screen_width: 10,
        screen_height: 3,
        content: "line 1\nline 2\nline 3",
        events: vec![key('k'), key('j'), key('q')],
    });
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
    let out = run_test(TestCase {
        screen_width: 10,
        screen_height: 4,
        content: "line 1\nline 2\nline 3\nline 4\nline 5\nline 6\nline 7\nline 8",
        events: vec![key('d'), key('u'), key('q')],
    });
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
fn scroll_with_soft_wrap() {
    // "abcdefgh" wraps to "abcde" + "fgh" at width 5.
    // Initial display shows them with soft wrap marker '>'.
    let out = run_test(TestCase {
        screen_width: 5,
        screen_height: 3,
        content: "short\nabcdefgh\nend",
        events: vec![key('j'), key('j'), key('k'), key('q')],
    });
    assert_eq!(
        out,
        "\
short
abcde>
fgh
-----
[EVENT]:char:j
abcde>
fgh
end
-----
[EVENT]:char:j
[EVENT]:char:k
short
abcde>
fgh
-----
[EVENT]:char:q
"
    );
}

#[test]
fn scroll_down_reveals_wrap_continuation() {
    // When scrolling down reveals a new wrap row, the entire visible
    // portion of that line is redrawn to maintain soft wraps.
    // Line "aaabbbccc" wraps to 3 rows at width 3.
    let out = run_test(TestCase {
        screen_width: 3,
        screen_height: 4,
        content: "xx\naaabbbccc\nyy",
        events: vec![key('j'), key('j'), key('q')],
    });
    // Initial: xx, aaa>, bbb>, ccc (line 1 wraps to 3 rows)
    // After j: aaa>, bbb>, ccc, yy
    // After j: bbb>, ccc, yy, (empty) - but yy is last, can't scroll
    assert_eq!(
        out,
        "\
xx
aaa>
bbb>
ccc
-----
[EVENT]:char:j
aaa>
bbb>
ccc
yy
-----
[EVENT]:char:j
[EVENT]:char:q
"
    );
}

#[test]
fn scroll_up_reveals_wrap_start() {
    // Scrolling up to reveal a new wrap row that has the same line as rows below.
    let out = run_test(TestCase {
        screen_width: 5,
        screen_height: 3,
        content: "xx\nabcdefgh\nyy",
        events: vec![key('j'), key('j'), key('k'), key('q')],
    });
    // "abcdefgh" wraps to "abcde" + "fgh"
    // Initial: xx, abcde>, fgh
    // j: abcde>, fgh, yy
    // j: fgh, yy, (empty) - can't scroll
    // k from [abcde>, fgh, yy]: back to [xx, abcde>, fgh]
    assert_eq!(
        out,
        "\
xx
abcde>
fgh
-----
[EVENT]:char:j
abcde>
fgh
yy
-----
[EVENT]:char:j
[EVENT]:char:k
xx
abcde>
fgh
-----
[EVENT]:char:q
"
    );
}

#[test]
fn wrap_midline_at_top_of_screen() {
    // When the top of the screen shows the middle of a wrapped line,
    // visible wrap rows should still be soft-wrapped together.
    // "abcdefghijk" wraps to "abcde" + "fghij" + "k" at width 5.
    let out = run_test(TestCase {
        screen_width: 5,
        screen_height: 3,
        content: "abcdefghijk\nend",
        events: vec![key('j'), key('q')],
    });
    // Initial: abcde>, fghij>, k
    // j: fghij>, k, end
    // Even though "abcde" is off-screen, "fghij" and "k" should be
    // soft-wrapped together.
    assert_eq!(
        out,
        "\
abcde>
fghij>
k
-----
[EVENT]:char:j
fghij>
k
end
-----
[EVENT]:char:q
"
    );
}

#[test]
fn jump_to_end_with_wrap() {
    let out = run_test(TestCase {
        screen_width: 5,
        screen_height: 3,
        content: "line1\nline2\nabcdefgh",
        events: vec![key('G'), key('g'), key('q')],
    });
    // "abcdefgh" wraps to "abcde" + "fgh"
    // Initial: line1, line2, abcde (fgh off-screen, no soft wrap)
    // G: line2, abcde>, fgh (both visible, soft wrapped)
    // g: line1, line2, abcde (fgh off-screen again)
    assert_eq!(
        out,
        "\
line1
line2
abcde
-----
[EVENT]:char:G
line2
abcde>
fgh
-----
[EVENT]:char:g
line1
line2
abcde
-----
[EVENT]:char:q
"
    );
}
