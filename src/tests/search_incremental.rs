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
{reverse}{bold}f{/reverse}{/bold}oo bar
line 4
line 5
/f█
-----
[EVENT]:char:o
{reverse}{bold}fo{/reverse}{/bold}o bar
line 4
line 5
/fo█
-----
[EVENT]:char:o
{reverse}{bold}foo{/reverse}{/bold} bar
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
{reverse}{bold}t{/reverse}{/bold}arge{reverse}{underline}{bold}t{/reverse}{/underline}{/bold} here
line 4
line 5
/t█
-----
[EVENT]:char:a
{reverse}{bold}ta{/reverse}{/bold}rget here
line 4
line 5
/ta█
-----
[EVENT]:char:r
{reverse}{bold}tar{/reverse}{/bold}get here
line 4
line 5
/tar█
-----
[EVENT]:char:g
{reverse}{bold}targ{/reverse}{/bold}et here
line 4
line 5
/targ█
-----
[EVENT]:char:e
{reverse}{bold}targe{/reverse}{/bold}t here
line 4
line 5
/targe█
-----
[EVENT]:char:t
{reverse}{bold}target{/reverse}{/bold} here
line 4
line 5
/target█
-----
[EVENT]:enter
{reverse}{bold}target{/reverse}{/bold} here
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
{reverse}{bold}f{/reverse}{/bold}oo 1
bar
{reverse}{underline}{bold}f{/reverse}{/underline}{/bold}oo 2
/f█
-----
[EVENT]:char:o
{reverse}{bold}fo{/reverse}{/bold}o 1
bar
{reverse}{underline}{bold}f{/reverse}{/underline}{/bold}{underline}{bold}o{/underline}{/bold}o 2
/fo█
-----
[EVENT]:char:o
{reverse}{bold}foo{/reverse}{/bold} 1
bar
{reverse}{underline}{bold}f{/reverse}{/underline}{/bold}{underline}{bold}oo{/underline}{/bold} 2
/foo█
-----
[EVENT]:enter
{reverse}{bold}foo{/reverse}{/bold} 1
bar
{reverse}{underline}{bold}f{/reverse}{/underline}{/bold}{underline}{bold}oo{/underline}{/bold} 2
:
-----
[EVENT]:char:n
{reverse}{bold}foo{/reverse}{/bold} 2
baz
{reverse}{underline}{bold}f{/reverse}{/underline}{/bold}{underline}{bold}oo{/underline}{/bold} 3
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
{reverse}{bold}t{/reverse}{/bold}arge{reverse}{underline}{bold}t{/reverse}{/underline}{/bold} here
/t█
-----
[EVENT]:char:a
line 3
line 4
{reverse}{bold}ta{/reverse}{/bold}rget here
/ta█
-----
[EVENT]:char:r
line 3
line 4
{reverse}{bold}tar{/reverse}{/bold}get here
/tar█
-----
[EVENT]:char:g
line 3
line 4
{reverse}{bold}targ{/reverse}{/bold}et here
/targ█
-----
[EVENT]:char:e
line 3
line 4
{reverse}{bold}targe{/reverse}{/bold}t here
/targe█
-----
[EVENT]:char:t
line 3
line 4
{reverse}{bold}target{/reverse}{/bold} here
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
{reverse}{bold}t{/reverse}{/bold}arge{reverse}{underline}{bold}t{/reverse}{/underline}{/bold} here
/t█
-----
[EVENT]:char:a
line 3
line 4
{reverse}{bold}ta{/reverse}{/bold}rget here
/ta█
-----
[EVENT]:char:r
line 3
line 4
{reverse}{bold}tar{/reverse}{/bold}get here
/tar█
-----
[EVENT]:char:g
line 3
line 4
{reverse}{bold}targ{/reverse}{/bold}et here
/targ█
-----
[EVENT]:char:e
line 3
line 4
{reverse}{bold}targe{/reverse}{/bold}t here
/targe█
-----
[EVENT]:char:t
line 3
line 4
{reverse}{bold}target{/reverse}{/bold} here
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
{reverse}{bold}a{/reverse}{/bold}b here
{reverse}{underline}{bold}a{/reverse}{/underline}{/bold}bc there
line 3
/a█
-----
[EVENT]:char:b
{reverse}{bold}ab{/reverse}{/bold} here
{reverse}{underline}{bold}a{/reverse}{/underline}{/bold}{underline}{bold}b{/underline}{/bold}c there
line 3
/ab█
-----
[EVENT]:char:c
{reverse}{bold}abc{/reverse}{/bold} there
line 3
line 4
/abc█
-----
[EVENT]:backspace
{reverse}{bold}ab{/reverse}{/bold} here
{reverse}{underline}{bold}a{/reverse}{/underline}{/bold}{underline}{bold}b{/underline}{/bold}c there
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
{reverse}{bold}f{/reverse}{/bold}oo here
line 2
line 3
/f█
-----
[EVENT]:char:o
{reverse}{bold}fo{/reverse}{/bold}o here
line 2
line 3
/fo█
-----
[EVENT]:char:o
{reverse}{bold}foo{/reverse}{/bold} here
line 2
line 3
/foo█
-----
[EVENT]:enter
{reverse}{bold}foo{/reverse}{/bold} here
line 2
line 3
:
-----
[EVENT]:char:/
{reverse}{bold}foo{/reverse}{/bold} here
line 2
line 3
/█
-----
[EVENT]:enter
{reverse}{bold}foo{/reverse}{/bold} here
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
{reverse}{bold}f{/reverse}{/bold}oo here
bar there
line 3
/f█
-----
[EVENT]:char:o
{reverse}{bold}fo{/reverse}{/bold}o here
bar there
line 3
/fo█
-----
[EVENT]:char:o
{reverse}{bold}foo{/reverse}{/bold} here
bar there
line 3
/foo█
-----
[EVENT]:enter
{reverse}{bold}foo{/reverse}{/bold} here
bar there
line 3
:
-----
[EVENT]:char:/
{reverse}{bold}foo{/reverse}{/bold} here
bar there
line 3
/█
-----
[EVENT]:char:b
{reverse}{bold}b{/reverse}{/bold}ar there
line 3
line 4
/b█
-----
[EVENT]:char:a
{reverse}{bold}ba{/reverse}{/bold}r there
line 3
line 4
/ba█
-----
[EVENT]:char:r
{reverse}{bold}bar{/reverse}{/bold} there
line 3
line 4
/bar█
-----
[EVENT]:esc
{reverse}{bold}foo{/reverse}{/bold} here
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
{reverse}{bold}f{/reverse}{/bold}{reverse}{underline}{bold}f{/reverse}{/underline}{/bold} bar
line 2
line 3
/f█
-----
[EVENT]:char:f
{reverse}{bold}ff{/reverse}{/bold} bar
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
// Current match (first) is reverse + bold; other matches are underline + bold
// with their first character also reversed.
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
{reverse}{bold}f{/reverse}{/bold}oo {reverse}{underline}{bold}f{/reverse}{/underline}{/bold}irst
{reverse}{underline}{bold}f{/reverse}{/underline}{/bold}oo second
{reverse}{underline}{bold}f{/reverse}{/underline}{/bold}oo third
/f█
-----
[EVENT]:char:o
{reverse}{bold}fo{/reverse}{/bold}o first
{reverse}{underline}{bold}f{/reverse}{/underline}{/bold}{underline}{bold}o{/underline}{/bold}o second
{reverse}{underline}{bold}f{/reverse}{/underline}{/bold}{underline}{bold}o{/underline}{/bold}o third
/fo█
-----
[EVENT]:char:o
{reverse}{bold}foo{/reverse}{/bold} first
{reverse}{underline}{bold}f{/reverse}{/underline}{/bold}{underline}{bold}oo{/underline}{/bold} second
{reverse}{underline}{bold}f{/reverse}{/underline}{/bold}{underline}{bold}oo{/underline}{/bold} third
/foo█
-----
[EVENT]:enter
{reverse}{bold}foo{/reverse}{/bold} first
{reverse}{underline}{bold}f{/reverse}{/underline}{/bold}{underline}{bold}oo{/underline}{/bold} second
{reverse}{underline}{bold}f{/reverse}{/underline}{/bold}{underline}{bold}oo{/underline}{/bold} third
:
-----
";
    assert_eq!(screen.out(), want);
}
