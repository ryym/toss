use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use pretty_assertions::assert_eq;

use super::{TestCase, key, run_test, run_test_screen};

fn enter() -> Event {
    Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
}

// Forward search: /foo + Enter jumps to the line containing "foo"
// and highlights the match with reverse video.
#[test]
fn forward_search_jumps_to_match() {
    let screen = run_test_screen(TestCase {
        screen_width: 20,
        screen_height: 4,
        content: "\
line 1
line 2
line 3
target foo here
line 5",
        events: vec![key('/'), key('f'), key('o'), key('o'), enter()],
    });
    assert_eq!(
        screen.last_snapshot(),
        "\
target {reverse}foo{/reverse} here
line 5
:

"
    );
}

// Backward search: ?top + Enter from the bottom jumps back to match.
#[test]
fn backward_search_jumps_to_match() {
    let out = run_test(TestCase {
        screen_width: 20,
        screen_height: 4,
        content: "\
top line
line 2
line 3
line 4
line 5",
        events: vec![
            key('G'), // jump to end
            key('?'),
            key('t'),
            key('o'),
            key('p'),
            enter(),
        ],
    });
    assert_eq!(
        out,
        "\
top line
line 2
line 3
:
-----
[EVENT]:char:G
line 3
line 4
line 5
:
-----
[EVENT]:char:?
line 3
line 4
line 5
?
-----
[EVENT]:char:t
line 3
line 4
line 5
?t
-----
[EVENT]:char:o
line 3
line 4
line 5
?to
-----
[EVENT]:char:p
line 3
line 4
line 5
?top
-----
[EVENT]:other
{reverse}top{/reverse} line
line 2
line 3
:
-----
"
    );
}

// n key: jump to next match in search direction.
#[test]
fn next_match_navigation() {
    let screen = run_test_screen(TestCase {
        screen_width: 20,
        screen_height: 4,
        content: "\
foo 1
bar
foo 2
baz
foo 3",
        events: vec![
            key('/'),
            key('f'),
            key('o'),
            key('o'),
            enter(),  // finds "foo 1" (line 0)
            key('n'), // next: "foo 2" (line 2)
        ],
    });
    assert_eq!(
        screen.last_snapshot(),
        "\
{reverse}foo{/reverse} 2
baz
{reverse}foo{/reverse} 3
:
"
    );
}

// N key: jump to previous match (reverse direction).
#[test]
fn prev_match_navigation() {
    let screen = run_test_screen(TestCase {
        screen_width: 20,
        screen_height: 4,
        content: "\
foo 1
bar
foo 2
baz
foo 3",
        events: vec![
            key('/'),
            key('f'),
            key('o'),
            key('o'),
            enter(),  // finds "foo 1" (line 0)
            key('N'), // previous (backward): wraps to "foo 3" (line 4)
        ],
    });
    assert_eq!(
        screen.last_snapshot(),
        "\
{reverse}foo{/reverse} 3
:


"
    );
}

// No match: position stays the same.
#[test]
fn no_match_stays_in_place() {
    let screen = run_test_screen(TestCase {
        screen_width: 20,
        screen_height: 4,
        content: "\
line 1
line 2
line 3
line 4",
        events: vec![key('/'), key('z'), key('z'), key('z'), enter()],
    });
    assert_eq!(
        screen.last_snapshot(),
        "\
line 1
line 2
line 3
:
"
    );
}

// Wrap around: search wraps from end to beginning.
#[test]
fn search_wraps_around() {
    let screen = run_test_screen(TestCase {
        screen_width: 20,
        screen_height: 4,
        content: "\
target here
line 2
line 3
line 4
line 5",
        events: vec![
            key('j'), // scroll down 1
            key('/'),
            key('t'),
            key('a'),
            key('r'),
            key('g'),
            key('e'),
            key('t'),
            enter(), // should wrap around to "target here" at line 0
        ],
    });
    assert_eq!(
        screen.last_snapshot(),
        "\
{reverse}target{/reverse} here
line 2
line 3
:
"
    );
}

// Highlighting with ANSI escape sequences: match spans across escape sequences.
#[test]
fn highlight_with_ansi_escapes() {
    let screen = run_test_screen(TestCase {
        screen_width: 30,
        screen_height: 4,
        content: "line 1\nThis is \x1b[1mCargo\x1b[0m.toml\nline 3\nline 4",
        events: vec![
            key('/'),
            key('C'),
            key('a'),
            key('r'),
            key('g'),
            key('o'),
            enter(),
        ],
    });
    // The bold start (\x1b[1m) precedes "Cargo" in raw text and the
    // reset (\x1b[0m) follows it. The match end position in raw text
    // falls after the reset, so the reset appears inside the highlighted span.
    assert_eq!(
        screen.last_snapshot(),
        "\
This is {bold}{reverse}Cargo{reset}{/reverse}.toml
line 3
line 4
:
"
    );
}

// Multiple matches on the same line are all highlighted.
#[test]
fn multiple_matches_same_line() {
    let screen = run_test_screen(TestCase {
        screen_width: 30,
        screen_height: 4,
        content: "\
foo bar foo baz foo
line 2
line 3
line 4",
        events: vec![key('/'), key('f'), key('o'), key('o'), enter()],
    });
    assert_eq!(
        screen.last_snapshot(),
        "\
{reverse}foo{/reverse} bar {reverse}foo{/reverse} baz {reverse}foo{/reverse}
line 2
line 3
:
"
    );
}
