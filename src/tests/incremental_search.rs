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

// Each keystroke updates the preview highlight and jumps to the first match.
#[test]
fn highlights_update_on_each_keystroke() {
    let out = run_test_screen(TestCase {
        screen_width: 20,
        screen_height: 4,
        content: "\
line 1
line 2
foo bar
line 4
line 5
line 6",
        events: vec![key('/'), key('f'), key('o'), key('o'), esc()],
    })
    .out();
    assert_eq!(
        out,
        "\
line 1
line 2
foo bar
:
-----
[EVENT]:char:/
line 1
line 2
foo bar
/
-----
[EVENT]:char:f
{reverse}f{/reverse}oo bar
line 4
line 5
/f
-----
[EVENT]:char:o
{reverse}fo{/reverse}o bar
line 4
line 5
/fo
-----
[EVENT]:char:o
{reverse}foo{/reverse} bar
line 4
line 5
/foo
-----
[EVENT]:esc
line 1
line 2
foo bar
:
-----
"
    );
}

// Enter commits the search preview; highlights persist in view mode.
#[test]
fn enter_commits_search() {
    let screen = run_test_screen(TestCase {
        screen_width: 20,
        screen_height: 4,
        content: "\
line 1
line 2
target here
line 4
line 5
line 6",
        events: vec![
            key('/'),
            key('t'),
            key('a'),
            key('r'),
            key('g'),
            key('e'),
            key('t'),
            enter(),
        ],
    });
    assert_eq!(
        screen.last_snapshot(),
        "\
{reverse}target{/reverse} here
line 4
line 5
:
"
    );
}

// After committing, n/N navigate between matches.
#[test]
fn committed_search_supports_navigation() {
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
            enter(),
            key('n'), // next: foo 2
        ],
    });
    assert_eq!(
        screen.last_snapshot(),
        "\
{reverse}foo{/reverse} 2
baz
{dim}{reverse}foo{/reverse}{/dim} 3
:
"
    );
}

// Esc cancels the search and restores the original scroll position.
#[test]
fn esc_cancels_and_restores_position() {
    let screen = run_test_screen(TestCase {
        screen_width: 20,
        screen_height: 4,
        content: "\
line 1
line 2
line 3
line 4
target here",
        events: vec![
            key('/'),
            key('t'),
            key('a'),
            key('r'),
            key('g'),
            key('e'),
            key('t'),
            esc(),
        ],
    });
    // Should be back to the original position with no highlights.
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

// Esc after scrolling restores the pre-search scroll position, not the top.
#[test]
fn esc_restores_scrolled_position() {
    let screen = run_test_screen(TestCase {
        screen_width: 20,
        screen_height: 4,
        content: "\
line 1
line 2
line 3
line 4
target here",
        events: vec![
            key('j'), // scroll down 1: now showing lines 2-4
            key('/'),
            key('t'),
            key('a'),
            key('r'),
            key('g'),
            key('e'),
            key('t'),
            esc(),
        ],
    });
    // Should restore to the position after scrolling (line 2 at top).
    assert_eq!(
        screen.last_snapshot(),
        "\
line 2
line 3
line 4
:
"
    );
}

// Backspace on empty input cancels search like Esc.
#[test]
fn backspace_on_empty_cancels_search() {
    let screen = run_test_screen(TestCase {
        screen_width: 20,
        screen_height: 4,
        content: "\
line 1
line 2
line 3
target here",
        events: vec![key('/'), backspace()],
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

// Backspace updates the preview to match the shorter query.
#[test]
fn backspace_updates_preview() {
    let out = run_test_screen(TestCase {
        screen_width: 20,
        screen_height: 4,
        content: "\
ab here
abc there
line 3
line 4",
        events: vec![
            key('/'),
            key('a'),
            key('b'),
            key('c'),
            backspace(), // "ab" again
            esc(),
        ],
    })
    .out();
    assert_eq!(
        out,
        "\
ab here
abc there
line 3
:
-----
[EVENT]:char:/
ab here
abc there
line 3
/
-----
[EVENT]:char:a
{reverse}a{/reverse}b here
{dim}{reverse}a{/reverse}{/dim}bc there
line 3
/a
-----
[EVENT]:char:b
{reverse}ab{/reverse} here
{dim}{reverse}ab{/reverse}{/dim}c there
line 3
/ab
-----
[EVENT]:char:c
{reverse}abc{/reverse} there
line 3
line 4
/abc
-----
[EVENT]:other
{reverse}ab{/reverse} here
{dim}{reverse}ab{/reverse}{/dim}c there
line 3
/ab
-----
[EVENT]:esc
ab here
abc there
line 3
:
-----
"
    );
}

// Enter on empty input exits search without changing committed state.
#[test]
fn enter_on_empty_does_not_commit() {
    let screen = run_test_screen(TestCase {
        screen_width: 20,
        screen_height: 4,
        content: "\
foo here
line 2
line 3
line 4",
        events: vec![
            key('/'),
            key('f'),
            key('o'),
            key('o'),
            enter(), // commit "foo"
            key('/'),
            enter(), // empty search, should keep previous "foo" highlight
        ],
    });
    assert_eq!(
        screen.last_snapshot(),
        "\
{reverse}foo{/reverse} here
line 2
line 3
:
"
    );
}

// Esc discards preview without affecting previous committed search.
#[test]
fn esc_preserves_previous_committed_search() {
    let screen = run_test_screen(TestCase {
        screen_width: 20,
        screen_height: 4,
        content: "\
foo here
bar there
line 3
line 4",
        events: vec![
            key('/'),
            key('f'),
            key('o'),
            key('o'),
            enter(), // commit "foo"
            key('/'),
            key('b'),
            key('a'),
            key('r'),
            esc(), // cancel "bar" search
        ],
    });
    // Previous "foo" search should still be active.
    assert_eq!(
        screen.last_snapshot(),
        "\
{reverse}foo{/reverse} here
bar there
line 3
:
"
    );
}

// Preview shows all matches on screen, not just the current one.
// Current match (first) is reverse, others are dim reverse.
#[test]
fn preview_highlights_all_visible_matches() {
    let screen = run_test_screen(TestCase {
        screen_width: 30,
        screen_height: 4,
        content: "\
foo first
foo second
foo third
line 4
line 5
line 6",
        events: vec![key('/'), key('f'), key('o'), key('o'), enter()],
    });
    assert_eq!(
        screen.last_snapshot(),
        "\
{reverse}foo{/reverse} first
{dim}{reverse}foo{/reverse}{/dim} second
{dim}{reverse}foo{/reverse}{/dim} third
:
"
    );
}
