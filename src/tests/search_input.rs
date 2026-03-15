use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use pretty_assertions::assert_eq;

use super::{TestCase, key, run_test_screen};

fn esc() -> Event {
    Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))
}

fn enter() -> Event {
    Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
}

fn backspace() -> Event {
    Event::Key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE))
}

// Typing / enters search mode and shows "/" on the status line.
// Typing characters triggers incremental search.
// Esc cancels and restores the original position.
#[test]
fn forward_search_input_and_cancel() {
    let out = run_test_screen(TestCase {
        screen_width: 10,
        screen_height: 4,
        content: "\
line 1
line 2
line 3
line 4",
        events: vec![key('/'), key('a'), key('b'), esc()],
        ..Default::default()
    })
    .out();
    // No match for "a" or "ab" so position stays the same.
    // Esc restores original position.
    assert_eq!(
        out,
        "\
line 1
line 2
line 3
:
-----
[EVENT]:char:/
line 1
line 2
line 3
/
-----
[EVENT]:char:a
line 1
line 2
line 3
/a
-----
[EVENT]:char:b
line 1
line 2
line 3
/ab
-----
[EVENT]:esc
line 1
line 2
line 3
:
-----
"
    );
}

// Typing ? enters backward search mode with "?" prompt.
// Enter submits and returns to view mode.
#[test]
fn backward_search_input_and_submit() {
    let out = run_test_screen(TestCase {
        screen_width: 10,
        screen_height: 4,
        content: "\
line 1
line 2
line 3
line 4",
        events: vec![key('?'), key('x'), enter()],
        ..Default::default()
    })
    .out();
    assert_eq!(
        out,
        "\
line 1
line 2
line 3
:
-----
[EVENT]:char:?
line 1
line 2
line 3
?
-----
[EVENT]:char:x
line 1
line 2
line 3
?x
-----
[EVENT]:other
line 1
line 2
line 3
:
-----
"
    );
}

// Backspace removes the last character from input.
#[test]
fn backspace_removes_character() {
    let out = run_test_screen(TestCase {
        screen_width: 10,
        screen_height: 4,
        content: "\
line 1
line 2
line 3
line 4",
        events: vec![key('/'), key('a'), key('b'), backspace(), esc()],
        ..Default::default()
    })
    .out();
    assert_eq!(
        out,
        "\
line 1
line 2
line 3
:
-----
[EVENT]:char:/
line 1
line 2
line 3
/
-----
[EVENT]:char:a
line 1
line 2
line 3
/a
-----
[EVENT]:char:b
line 1
line 2
line 3
/ab
-----
[EVENT]:other
line 1
line 2
line 3
/a
-----
[EVENT]:esc
line 1
line 2
line 3
:
-----
"
    );
}
