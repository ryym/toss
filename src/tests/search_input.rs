use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use pretty_assertions::assert_eq;

use super::{TestCase, key, run_test_screen};

fn left() -> Event {
    Event::Key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE))
}

fn right() -> Event {
    Event::Key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE))
}

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
    let content = "\
line 1
line 2
line 3
line 4
";
    let screen = run_test_screen(TestCase {
        screen_width: 10,
        screen_height: 4,
        content,
        events: vec![key('/'), key('a'), key('b'), esc()],
        ..Default::default()
    });
    // No match for "a" or "ab" so position stays the same.
    // Esc restores original position.
    let want = "\
line 1
line 2
line 3
{rev} 1-3/4 75%{/rev}
-----
[EVENT]:char:/
line 1
line 2
line 3
/█
-----
[EVENT]:char:a
line 1
line 2
line 3
/a█
-----
[EVENT]:char:b
line 1
line 2
line 3
/ab█
-----
[EVENT]:esc
line 1
line 2
line 3
{rev} 1-3/4 75%{/rev}
-----
";
    assert_eq!(screen.out(), want);
}

// Typing ? enters backward search mode with "?" prompt.
// Enter submits and returns to view mode.
#[test]
fn backward_search_input_and_submit() {
    let content = "\
line 1
line 2
line 3
line 4
";
    let screen = run_test_screen(TestCase {
        screen_width: 10,
        screen_height: 4,
        content,
        events: vec![key('?'), key('x'), enter()],
        ..Default::default()
    });
    let want = "\
line 1
line 2
line 3
{rev} 1-3/4 75%{/rev}
-----
[EVENT]:char:?
line 1
line 2
line 3
?█
-----
[EVENT]:char:x
line 1
line 2
line 3
?x█
-----
[EVENT]:enter
line 1
line 2
line 3
{rev} 1-3/4 75%{/rev}
-----
";
    assert_eq!(screen.out(), want);
}

// Backspace removes the last character from input.
#[test]
fn backspace_removes_character() {
    let content = "\
line 1
line 2
line 3
line 4
";
    let screen = run_test_screen(TestCase {
        screen_width: 10,
        screen_height: 4,
        content,
        events: vec![key('/'), key('a'), key('b'), backspace(), esc()],
        ..Default::default()
    });
    let want = "\
line 1
line 2
line 3
{rev} 1-3/4 75%{/rev}
-----
[EVENT]:char:/
line 1
line 2
line 3
/█
-----
[EVENT]:char:a
line 1
line 2
line 3
/a█
-----
[EVENT]:char:b
line 1
line 2
line 3
/ab█
-----
[EVENT]:backspace
line 1
line 2
line 3
/a█
-----
[EVENT]:esc
line 1
line 2
line 3
{rev} 1-3/4 75%{/rev}
-----
";
    assert_eq!(screen.out(), want);
}

// Arrow keys move the cursor within the search input.
#[test]
fn arrow_keys_move_cursor() {
    let content = "\
line 1
line 2
line 3
line 4
";
    let screen = run_test_screen(TestCase {
        screen_width: 10,
        screen_height: 4,
        content,
        events: vec![
            key('/'),
            key('a'),
            key('b'),
            key('c'),
            left(),
            left(),
            key('x'),
            right(),
            esc(),
        ],
        ..Default::default()
    });
    let want = "\
line 1
line 2
line 3
{rev} 1-3/4 75%{/rev}
-----
[EVENT]:char:/
line 1
line 2
line 3
/█
-----
[EVENT]:char:a
line 1
line 2
line 3
/a█
-----
[EVENT]:char:b
line 1
line 2
line 3
/ab█
-----
[EVENT]:char:c
line 1
line 2
line 3
/abc█
-----
[EVENT]:left
line 1
line 2
line 3
/ab█c
-----
[EVENT]:left
line 1
line 2
line 3
/a█bc
-----
[EVENT]:char:x
line 1
line 2
line 3
/ax█bc
-----
[EVENT]:right
line 1
line 2
line 3
/axb█c
-----
[EVENT]:esc
line 1
line 2
line 3
{rev} 1-3/4 75%{/rev}
-----
";
    assert_eq!(screen.out(), want);
}
