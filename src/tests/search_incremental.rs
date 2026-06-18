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
{rev}lines 1-3/6 50%{/rev}
-----
[EVENT]:char:/
line 1
line 2
foo bar
/█
-----
[EVENT]:char:f
{rev}{b}f{/rev}{/b}oo bar
line 4
line 5
/f█
-----
[EVENT]:char:o
{rev}{b}fo{/rev}{/b}o bar
line 4
line 5
/fo█
-----
[EVENT]:char:o
{rev}{b}foo{/rev}{/b} bar
line 4
line 5
/foo█
-----
[EVENT]:esc
line 1
line 2
foo bar
{rev}lines 1-3/6 50%{/rev}
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
{rev}lines 1-3/6 50%{/rev}
-----
[EVENT]:char:/
line 1
line 2
target here
/█
-----
[EVENT]:char:t
{rev}{b}t{/rev}{/b}arge{rev}{line}{b}t{/rev}{/line}{/b} here
line 4
line 5
/t█
-----
[EVENT]:char:a
{rev}{b}ta{/rev}{/b}rget here
line 4
line 5
/ta█
-----
[EVENT]:char:r
{rev}{b}tar{/rev}{/b}get here
line 4
line 5
/tar█
-----
[EVENT]:char:g
{rev}{b}targ{/rev}{/b}et here
line 4
line 5
/targ█
-----
[EVENT]:char:e
{rev}{b}targe{/rev}{/b}t here
line 4
line 5
/targe█
-----
[EVENT]:char:t
{rev}{b}target{/rev}{/b} here
line 4
line 5
/target█
-----
[EVENT]:enter
{rev}{b}target{/rev}{/b} here
line 4
line 5
{rev}lines 3-5/6 83%{/rev}
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
{rev}lines 1-3/5 60%{/rev}
-----
[EVENT]:char:/
foo 1
bar
foo 2
/█
-----
[EVENT]:char:f
{rev}{b}f{/rev}{/b}oo 1
bar
{rev}{line}{b}f{/rev}{/line}{/b}oo 2
/f█
-----
[EVENT]:char:o
{rev}{b}fo{/rev}{/b}o 1
bar
{rev}{line}{b}f{/rev}{/line}{/b}{line}{b}o{/line}{/b}o 2
/fo█
-----
[EVENT]:char:o
{rev}{b}foo{/rev}{/b} 1
bar
{rev}{line}{b}f{/rev}{/line}{/b}{line}{b}oo{/line}{/b} 2
/foo█
-----
[EVENT]:enter
{rev}{b}foo{/rev}{/b} 1
bar
{rev}{line}{b}f{/rev}{/line}{/b}{line}{b}oo{/line}{/b} 2
{rev}lines 1-3/5 60%{/rev}
-----
[EVENT]:char:n
{rev}{line}{b}f{/rev}{/line}{/b}{line}{b}oo{/line}{/b} 1
bar
{rev}{b}foo{/rev}{/b} 2
{rev}lines 1-3/5 60%{/rev}
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
{rev}lines 1-3/5 60%{/rev}
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
{rev}{b}t{/rev}{/b}arge{rev}{line}{b}t{/rev}{/line}{/b} here
/t█
-----
[EVENT]:char:a
line 3
line 4
{rev}{b}ta{/rev}{/b}rget here
/ta█
-----
[EVENT]:char:r
line 3
line 4
{rev}{b}tar{/rev}{/b}get here
/tar█
-----
[EVENT]:char:g
line 3
line 4
{rev}{b}targ{/rev}{/b}et here
/targ█
-----
[EVENT]:char:e
line 3
line 4
{rev}{b}targe{/rev}{/b}t here
/targe█
-----
[EVENT]:char:t
line 3
line 4
{rev}{b}target{/rev}{/b} here
/target█
-----
[EVENT]:esc
line 1
line 2
line 3
{rev}lines 1-3/5 60%{/rev}
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
{rev}lines 1-3/5 60%{/rev}
-----
[EVENT]:char:j
line 2
line 3
line 4
{rev}lines 2-4/5 80%{/rev}
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
{rev}{b}t{/rev}{/b}arge{rev}{line}{b}t{/rev}{/line}{/b} here
/t█
-----
[EVENT]:char:a
line 3
line 4
{rev}{b}ta{/rev}{/b}rget here
/ta█
-----
[EVENT]:char:r
line 3
line 4
{rev}{b}tar{/rev}{/b}get here
/tar█
-----
[EVENT]:char:g
line 3
line 4
{rev}{b}targ{/rev}{/b}et here
/targ█
-----
[EVENT]:char:e
line 3
line 4
{rev}{b}targe{/rev}{/b}t here
/targe█
-----
[EVENT]:char:t
line 3
line 4
{rev}{b}target{/rev}{/b} here
/target█
-----
[EVENT]:esc
line 2
line 3
line 4
{rev}lines 2-4/5 80%{/rev}
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
{rev}lines 1-3/4 75%{/rev}
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
{rev}lines 1-3/4 75%{/rev}
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
{rev}lines 1-3/4 75%{/rev}
-----
[EVENT]:char:/
ab here
abc there
line 3
/█
-----
[EVENT]:char:a
{rev}{b}a{/rev}{/b}b here
{rev}{line}{b}a{/rev}{/line}{/b}bc there
line 3
/a█
-----
[EVENT]:char:b
{rev}{b}ab{/rev}{/b} here
{rev}{line}{b}a{/rev}{/line}{/b}{line}{b}b{/line}{/b}c there
line 3
/ab█
-----
[EVENT]:char:c
{rev}{b}abc{/rev}{/b} there
line 3
line 4
/abc█
-----
[EVENT]:backspace
{rev}{b}ab{/rev}{/b} here
{rev}{line}{b}a{/rev}{/line}{/b}{line}{b}b{/line}{/b}c there
line 3
/ab█
-----
[EVENT]:esc
ab here
abc there
line 3
{rev}lines 1-3/4 75%{/rev}
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
{rev}lines 1-3/4 75%{/rev}
-----
[EVENT]:char:/
foo here
line 2
line 3
/█
-----
[EVENT]:char:f
{rev}{b}f{/rev}{/b}oo here
line 2
line 3
/f█
-----
[EVENT]:char:o
{rev}{b}fo{/rev}{/b}o here
line 2
line 3
/fo█
-----
[EVENT]:char:o
{rev}{b}foo{/rev}{/b} here
line 2
line 3
/foo█
-----
[EVENT]:enter
{rev}{b}foo{/rev}{/b} here
line 2
line 3
{rev}lines 1-3/4 75%{/rev}
-----
[EVENT]:char:/
{rev}{b}foo{/rev}{/b} here
line 2
line 3
/█
-----
[EVENT]:enter
{rev}{b}foo{/rev}{/b} here
line 2
line 3
{rev}lines 1-3/4 75%{/rev}
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
{rev}lines 1-3/4 75%{/rev}
-----
[EVENT]:char:/
foo here
bar there
line 3
/█
-----
[EVENT]:char:f
{rev}{b}f{/rev}{/b}oo here
bar there
line 3
/f█
-----
[EVENT]:char:o
{rev}{b}fo{/rev}{/b}o here
bar there
line 3
/fo█
-----
[EVENT]:char:o
{rev}{b}foo{/rev}{/b} here
bar there
line 3
/foo█
-----
[EVENT]:enter
{rev}{b}foo{/rev}{/b} here
bar there
line 3
{rev}lines 1-3/4 75%{/rev}
-----
[EVENT]:char:/
{rev}{b}foo{/rev}{/b} here
bar there
line 3
/█
-----
[EVENT]:char:b
{rev}{b}b{/rev}{/b}ar there
line 3
line 4
/b█
-----
[EVENT]:char:a
{rev}{b}ba{/rev}{/b}r there
line 3
line 4
/ba█
-----
[EVENT]:char:r
{rev}{b}bar{/rev}{/b} there
line 3
line 4
/bar█
-----
[EVENT]:esc
{rev}{b}foo{/rev}{/b} here
bar there
line 3
{rev}lines 1-3/4 75%{/rev}
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
{rev}lines 1-3/4 75%{/rev}
-----
[EVENT]:char:/
ff bar
line 2
line 3
/█
-----
[EVENT]:char:f
{rev}{b}f{/rev}{/b}{rev}{line}{b}f{/rev}{/line}{/b} bar
line 2
line 3
/f█
-----
[EVENT]:char:f
{rev}{b}ff{/rev}{/b} bar
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
{rev}lines 1-3/4 75%{/rev}
-----
";
    assert_eq!(screen.out(), want);
}

// Starting a new search after a committed one must not leave the previous
// query's highlights during the incremental preview of the new query.
#[test]
fn new_search_preview_clears_previous_committed_highlight() {
    // "bar" matches the first line so the preview jump does not scroll, while
    // "foo" has extra (underline) matches on the lines below that stay visible.
    let content = "\
bar aaa foo
foo bbb
foo ccc
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
            esc(),
        ],
        ..Default::default()
    });
    // During the "bar" preview, the leftover "foo" matches (including the
    // underline match on "foo ccc") must be cleared.
    let want = "\
bar aaa foo
foo bbb
foo ccc
{rev}lines 1-3/4 75%{/rev}
-----
[EVENT]:char:/
bar aaa foo
foo bbb
foo ccc
/█
-----
[EVENT]:char:f
bar aaa {rev}{b}f{/rev}{/b}oo
{rev}{line}{b}f{/rev}{/line}{/b}oo bbb
{rev}{line}{b}f{/rev}{/line}{/b}oo ccc
/f█
-----
[EVENT]:char:o
bar aaa {rev}{b}fo{/rev}{/b}o
{rev}{line}{b}f{/rev}{/line}{/b}{line}{b}o{/line}{/b}o bbb
{rev}{line}{b}f{/rev}{/line}{/b}{line}{b}o{/line}{/b}o ccc
/fo█
-----
[EVENT]:char:o
bar aaa {rev}{b}foo{/rev}{/b}
{rev}{line}{b}f{/rev}{/line}{/b}{line}{b}oo{/line}{/b} bbb
{rev}{line}{b}f{/rev}{/line}{/b}{line}{b}oo{/line}{/b} ccc
/foo█
-----
[EVENT]:enter
bar aaa {rev}{b}foo{/rev}{/b}
{rev}{line}{b}f{/rev}{/line}{/b}{line}{b}oo{/line}{/b} bbb
{rev}{line}{b}f{/rev}{/line}{/b}{line}{b}oo{/line}{/b} ccc
{rev}lines 1-3/4 75%{/rev}
-----
[EVENT]:char:/
bar aaa {rev}{b}foo{/rev}{/b}
{rev}{line}{b}f{/rev}{/line}{/b}{line}{b}oo{/line}{/b} bbb
{rev}{line}{b}f{/rev}{/line}{/b}{line}{b}oo{/line}{/b} ccc
/█
-----
[EVENT]:char:b
{rev}{b}b{/rev}{/b}ar aaa foo
foo {rev}{line}{b}b{/rev}{/line}{/b}{rev}{line}{b}b{/rev}{/line}{/b}{rev}{line}{b}b{/rev}{/line}{/b}
foo ccc
/b█
-----
[EVENT]:char:a
{rev}{b}ba{/rev}{/b}r aaa foo
foo bbb
foo ccc
/ba█
-----
[EVENT]:char:r
{rev}{b}bar{/rev}{/b} aaa foo
foo bbb
foo ccc
/bar█
-----
[EVENT]:esc
bar aaa {rev}{b}foo{/rev}{/b}
{rev}{line}{b}f{/rev}{/line}{/b}{line}{b}oo{/line}{/b} bbb
{rev}{line}{b}f{/rev}{/line}{/b}{line}{b}oo{/line}{/b} ccc
{rev}lines 1-3/4 75%{/rev}
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
{rev}lines 1-3/6 50%{/rev}
-----
[EVENT]:char:/
foo first
foo second
foo third
/█
-----
[EVENT]:char:f
{rev}{b}f{/rev}{/b}oo {rev}{line}{b}f{/rev}{/line}{/b}irst
{rev}{line}{b}f{/rev}{/line}{/b}oo second
{rev}{line}{b}f{/rev}{/line}{/b}oo third
/f█
-----
[EVENT]:char:o
{rev}{b}fo{/rev}{/b}o first
{rev}{line}{b}f{/rev}{/line}{/b}{line}{b}o{/line}{/b}o second
{rev}{line}{b}f{/rev}{/line}{/b}{line}{b}o{/line}{/b}o third
/fo█
-----
[EVENT]:char:o
{rev}{b}foo{/rev}{/b} first
{rev}{line}{b}f{/rev}{/line}{/b}{line}{b}oo{/line}{/b} second
{rev}{line}{b}f{/rev}{/line}{/b}{line}{b}oo{/line}{/b} third
/foo█
-----
[EVENT]:enter
{rev}{b}foo{/rev}{/b} first
{rev}{line}{b}f{/rev}{/line}{/b}{line}{b}oo{/line}{/b} second
{rev}{line}{b}f{/rev}{/line}{/b}{line}{b}oo{/line}{/b} third
{rev}lines 1-3/6 50%{/rev}
-----
";
    assert_eq!(screen.out(), want);
}
