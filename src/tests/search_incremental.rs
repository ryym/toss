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
    let content = "\
line 1
line 2
foo bar
line 4
line 5
line 6
";
    let screen = run_test_screen(TestCase {
        screen_width: 20,
        screen_height: 4,
        content,
        events: vec![key('/'), key('f'), key('o'), key('o'), esc()],
        ..Default::default()
    });
    let want = "\
line 1
line 2
foo bar
:
-----
[EVENT]:char:/
line 1
line 2
foo bar
/█
-----
[EVENT]:char:f
{reverse}f{/reverse}oo bar
line 4
line 5
/f█
-----
[EVENT]:char:o
{reverse}fo{/reverse}o bar
line 4
line 5
/fo█
-----
[EVENT]:char:o
{reverse}foo{/reverse} bar
line 4
line 5
/foo█
-----
[EVENT]:esc
line 1
line 2
foo bar
:
-----
";
    assert_eq!(screen.out(), want);
}

// Enter commits the search preview; highlights persist in view mode.
#[test]
fn enter_commits_search() {
    let content = "\
line 1
line 2
target here
line 4
line 5
line 6
";
    let screen = run_test_screen(TestCase {
        screen_width: 20,
        screen_height: 4,
        content,
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
        ..Default::default()
    });
    let want = "\
line 1
line 2
target here
:
-----
[EVENT]:char:/
line 1
line 2
target here
/█
-----
[EVENT]:char:t
{reverse}t{/reverse}arge{dim}{reverse}t{/reverse}{/dim} here
line 4
line 5
/t█
-----
[EVENT]:char:a
{reverse}ta{/reverse}rget here
line 4
line 5
/ta█
-----
[EVENT]:char:r
{reverse}tar{/reverse}get here
line 4
line 5
/tar█
-----
[EVENT]:char:g
{reverse}targ{/reverse}et here
line 4
line 5
/targ█
-----
[EVENT]:char:e
{reverse}targe{/reverse}t here
line 4
line 5
/targe█
-----
[EVENT]:char:t
{reverse}target{/reverse} here
line 4
line 5
/target█
-----
[EVENT]:enter
{reverse}target{/reverse} here
line 4
line 5
:
-----
";
    assert_eq!(screen.out(), want);
}

// After committing, n/N navigate between matches.
#[test]
fn committed_search_supports_navigation() {
    let content = "\
foo 1
bar
foo 2
baz
foo 3
";
    let screen = run_test_screen(TestCase {
        screen_width: 20,
        screen_height: 4,
        content,
        events: vec![
            key('/'),
            key('f'),
            key('o'),
            key('o'),
            enter(),
            key('n'), // next: foo 2
        ],
        ..Default::default()
    });
    let want = "\
foo 1
bar
foo 2
:
-----
[EVENT]:char:/
foo 1
bar
foo 2
/█
-----
[EVENT]:char:f
{reverse}f{/reverse}oo 1
bar
{dim}{reverse}f{/reverse}{/dim}oo 2
/f█
-----
[EVENT]:char:o
{reverse}fo{/reverse}o 1
bar
{dim}{reverse}fo{/reverse}{/dim}o 2
/fo█
-----
[EVENT]:char:o
{reverse}foo{/reverse} 1
bar
{dim}{reverse}foo{/reverse}{/dim} 2
/foo█
-----
[EVENT]:enter
{reverse}foo{/reverse} 1
bar
{dim}{reverse}foo{/reverse}{/dim} 2
:
-----
[EVENT]:char:n
{reverse}foo{/reverse} 2
baz
{dim}{reverse}foo{/reverse}{/dim} 3
:
-----
";
    assert_eq!(screen.out(), want);
}

// Esc cancels the search and restores the original scroll position.
#[test]
fn esc_cancels_and_restores_position() {
    let content = "\
line 1
line 2
line 3
line 4
target here
";
    let screen = run_test_screen(TestCase {
        screen_width: 20,
        screen_height: 4,
        content,
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
        ..Default::default()
    });
    // Should be back to the original position with no highlights.
    let want = "\
line 1
line 2
line 3
:
-----
[EVENT]:char:/
line 1
line 2
line 3
/█
-----
[EVENT]:char:t
line 3
line 4
{reverse}t{/reverse}arge{dim}{reverse}t{/reverse}{/dim} here
/t█
-----
[EVENT]:char:a
line 3
line 4
{reverse}ta{/reverse}rget here
/ta█
-----
[EVENT]:char:r
line 3
line 4
{reverse}tar{/reverse}get here
/tar█
-----
[EVENT]:char:g
line 3
line 4
{reverse}targ{/reverse}et here
/targ█
-----
[EVENT]:char:e
line 3
line 4
{reverse}targe{/reverse}t here
/targe█
-----
[EVENT]:char:t
line 3
line 4
{reverse}target{/reverse} here
/target█
-----
[EVENT]:esc
line 1
line 2
line 3
:
-----
";
    assert_eq!(screen.out(), want);
}

// Esc after scrolling restores the pre-search scroll position, not the top.
#[test]
fn esc_restores_scrolled_position() {
    let content = "\
line 1
line 2
line 3
line 4
target here
";
    let screen = run_test_screen(TestCase {
        screen_width: 20,
        screen_height: 4,
        content,
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
        ..Default::default()
    });
    // Should restore to the position after scrolling (line 2 at top).
    let want = "\
line 1
line 2
line 3
:
-----
[EVENT]:char:j
line 2
line 3
line 4
:
-----
[EVENT]:char:/
line 2
line 3
line 4
/█
-----
[EVENT]:char:t
line 3
line 4
{reverse}t{/reverse}arge{dim}{reverse}t{/reverse}{/dim} here
/t█
-----
[EVENT]:char:a
line 3
line 4
{reverse}ta{/reverse}rget here
/ta█
-----
[EVENT]:char:r
line 3
line 4
{reverse}tar{/reverse}get here
/tar█
-----
[EVENT]:char:g
line 3
line 4
{reverse}targ{/reverse}et here
/targ█
-----
[EVENT]:char:e
line 3
line 4
{reverse}targe{/reverse}t here
/targe█
-----
[EVENT]:char:t
line 3
line 4
{reverse}target{/reverse} here
/target█
-----
[EVENT]:esc
line 2
line 3
line 4
:
-----
";
    assert_eq!(screen.out(), want);
}

// Backspace on empty input cancels search like Esc.
#[test]
fn backspace_on_empty_cancels_search() {
    let content = "\
line 1
line 2
line 3
target here
";
    let screen = run_test_screen(TestCase {
        screen_width: 20,
        screen_height: 4,
        content,
        events: vec![key('/'), backspace()],
        ..Default::default()
    });
    let want = "\
line 1
line 2
line 3
:
-----
[EVENT]:char:/
line 1
line 2
line 3
/█
-----
[EVENT]:backspace
line 1
line 2
line 3
:
-----
";
    assert_eq!(screen.out(), want);
}

// Backspace updates the preview to match the shorter query.
#[test]
fn backspace_updates_preview() {
    let content = "\
ab here
abc there
line 3
line 4
";
    let screen = run_test_screen(TestCase {
        screen_width: 20,
        screen_height: 4,
        content,
        events: vec![
            key('/'),
            key('a'),
            key('b'),
            key('c'),
            backspace(), // "ab" again
            esc(),
        ],
        ..Default::default()
    });
    let want = "\
ab here
abc there
line 3
:
-----
[EVENT]:char:/
ab here
abc there
line 3
/█
-----
[EVENT]:char:a
{reverse}a{/reverse}b here
{dim}{reverse}a{/reverse}{/dim}bc there
line 3
/a█
-----
[EVENT]:char:b
{reverse}ab{/reverse} here
{dim}{reverse}ab{/reverse}{/dim}c there
line 3
/ab█
-----
[EVENT]:char:c
{reverse}abc{/reverse} there
line 3
line 4
/abc█
-----
[EVENT]:backspace
{reverse}ab{/reverse} here
{dim}{reverse}ab{/reverse}{/dim}c there
line 3
/ab█
-----
[EVENT]:esc
ab here
abc there
line 3
:
-----
";
    assert_eq!(screen.out(), want);
}

// Enter on empty input exits search without changing committed state.
#[test]
fn enter_on_empty_does_not_commit() {
    let content = "\
foo here
line 2
line 3
line 4
";
    let screen = run_test_screen(TestCase {
        screen_width: 20,
        screen_height: 4,
        content,
        events: vec![
            key('/'),
            key('f'),
            key('o'),
            key('o'),
            enter(), // commit "foo"
            key('/'),
            enter(), // empty search, should keep previous "foo" highlight
        ],
        ..Default::default()
    });
    let want = "\
foo here
line 2
line 3
:
-----
[EVENT]:char:/
foo here
line 2
line 3
/█
-----
[EVENT]:char:f
{reverse}f{/reverse}oo here
line 2
line 3
/f█
-----
[EVENT]:char:o
{reverse}fo{/reverse}o here
line 2
line 3
/fo█
-----
[EVENT]:char:o
{reverse}foo{/reverse} here
line 2
line 3
/foo█
-----
[EVENT]:enter
{reverse}foo{/reverse} here
line 2
line 3
:
-----
[EVENT]:char:/
{reverse}foo{/reverse} here
line 2
line 3
/█
-----
[EVENT]:enter
{reverse}foo{/reverse} here
line 2
line 3
:
-----
";
    assert_eq!(screen.out(), want);
}

// Esc discards preview without affecting previous committed search.
#[test]
fn esc_preserves_previous_committed_search() {
    let content = "\
foo here
bar there
line 3
line 4
";
    let screen = run_test_screen(TestCase {
        screen_width: 20,
        screen_height: 4,
        content,
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
        ..Default::default()
    });
    // Previous "foo" search should still be active.
    let want = "\
foo here
bar there
line 3
:
-----
[EVENT]:char:/
foo here
bar there
line 3
/█
-----
[EVENT]:char:f
{reverse}f{/reverse}oo here
bar there
line 3
/f█
-----
[EVENT]:char:o
{reverse}fo{/reverse}o here
bar there
line 3
/fo█
-----
[EVENT]:char:o
{reverse}foo{/reverse} here
bar there
line 3
/foo█
-----
[EVENT]:enter
{reverse}foo{/reverse} here
bar there
line 3
:
-----
[EVENT]:char:/
{reverse}foo{/reverse} here
bar there
line 3
/█
-----
[EVENT]:char:b
{reverse}b{/reverse}ar there
line 3
line 4
/b█
-----
[EVENT]:char:a
{reverse}ba{/reverse}r there
line 3
line 4
/ba█
-----
[EVENT]:char:r
{reverse}bar{/reverse} there
line 3
line 4
/bar█
-----
[EVENT]:esc
{reverse}foo{/reverse} here
bar there
line 3
:
-----
";
    assert_eq!(screen.out(), want);
}

// When the query stops matching, the previous match highlight should disappear.
#[test]
fn preview_clears_highlight_when_query_no_longer_matches() {
    let content = "\
ff bar
line 2
line 3
line 4
";
    let screen = run_test_screen(TestCase {
        screen_width: 20,
        screen_height: 4,
        content,
        events: vec![key('/'), key('f'), key('f'), key('f'), esc()],
        ..Default::default()
    });
    let want = "\
ff bar
line 2
line 3
:
-----
[EVENT]:char:/
ff bar
line 2
line 3
/█
-----
[EVENT]:char:f
{reverse}f{/reverse}{dim}{reverse}f{/reverse}{/dim} bar
line 2
line 3
/f█
-----
[EVENT]:char:f
{reverse}ff{/reverse} bar
line 2
line 3
/ff█
-----
[EVENT]:char:f
ff bar
line 2
line 3
/fff█
-----
[EVENT]:esc
ff bar
line 2
line 3
:
-----
";
    assert_eq!(screen.out(), want);
}

// Preview shows all matches on screen, not just the current one.
// Current match (first) is reverse, others are dim reverse.
#[test]
fn preview_highlights_all_visible_matches() {
    let content = "\
foo first
foo second
foo third
line 4
line 5
line 6
";
    let screen = run_test_screen(TestCase {
        screen_width: 30,
        screen_height: 4,
        content,
        events: vec![key('/'), key('f'), key('o'), key('o'), enter()],
        ..Default::default()
    });
    let want = "\
foo first
foo second
foo third
:
-----
[EVENT]:char:/
foo first
foo second
foo third
/█
-----
[EVENT]:char:f
{reverse}f{/reverse}oo {dim}{reverse}f{/reverse}{/dim}irst
{dim}{reverse}f{/reverse}{/dim}oo second
{dim}{reverse}f{/reverse}{/dim}oo third
/f█
-----
[EVENT]:char:o
{reverse}fo{/reverse}o first
{dim}{reverse}fo{/reverse}{/dim}o second
{dim}{reverse}fo{/reverse}{/dim}o third
/fo█
-----
[EVENT]:char:o
{reverse}foo{/reverse} first
{dim}{reverse}foo{/reverse}{/dim} second
{dim}{reverse}foo{/reverse}{/dim} third
/foo█
-----
[EVENT]:enter
{reverse}foo{/reverse} first
{dim}{reverse}foo{/reverse}{/dim} second
{dim}{reverse}foo{/reverse}{/dim} third
:
-----
";
    assert_eq!(screen.out(), want);
}
