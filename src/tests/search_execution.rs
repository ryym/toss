use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use pretty_assertions::assert_eq;

use super::{TestCase, key, run_test_screen};

fn enter() -> Event {
    Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
}

// Forward search: /foo + Enter jumps to the line containing "foo"
// and highlights the match with reverse video.
#[test]
fn forward_search_jumps_to_match() {
    let content = "\
line 1
line 2
line 3
target foo here
line 5
";
    let screen = run_test_screen(TestCase {
        screen_width: 20,
        screen_height: 4,
        content,
        events: vec![key('/'), key('f'), key('o'), key('o'), enter()],
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
[EVENT]:char:f
line 3
target {rev}{b}f{/rev}{/b}oo here
line 5
/f█
-----
[EVENT]:char:o
line 3
target {rev}{b}fo{/rev}{/b}o here
line 5
/fo█
-----
[EVENT]:char:o
line 3
target {rev}{b}foo{/rev}{/b} here
line 5
/foo█
-----
[EVENT]:enter
line 3
target {rev}{b}foo{/rev}{/b} here
line 5
:
-----
";
    assert_eq!(screen.out(), want);
}

// Backward search: ?top + Enter from the bottom jumps back to match.
// With incremental search, each keystroke triggers a search and jump.
#[test]
fn backward_search_jumps_to_match() {
    let content = "\
top line
line 2
line 3
line 4
line 5
";
    let out = run_test_screen(TestCase {
        screen_width: 20,
        screen_height: 4,
        content,
        events: vec![
            key('G'), // jump to end
            key('?'),
            key('t'),
            key('o'),
            key('p'),
            enter(),
        ],
        ..Default::default()
    })
    .out();
    let want = "\
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
?█
-----
[EVENT]:char:t
{rev}{b}t{/rev}{/b}op line
line 2
line 3
?t█
-----
[EVENT]:char:o
{rev}{b}to{/rev}{/b}p line
line 2
line 3
?to█
-----
[EVENT]:char:p
{rev}{b}top{/rev}{/b} line
line 2
line 3
?top█
-----
[EVENT]:enter
{rev}{b}top{/rev}{/b} line
line 2
line 3
:
-----
";
    assert_eq!(out, want);
}

// n key: jump to next match in search direction.
// Current match uses reverse + bold; other matches use underline + bold with
// their first character also reversed.
#[test]
fn next_match_navigation() {
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
            enter(),  // finds "foo 1" (line 0)
            key('n'), // next: "foo 2" (line 2)
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
:
-----
[EVENT]:char:n
{rev}{line}{b}f{/rev}{/line}{/b}{line}{b}oo{/line}{/b} 1
bar
{rev}{b}foo{/rev}{/b} 2
:
-----
";
    assert_eq!(screen.out(), want);
}

// N key: jump to previous match (reverse direction).
#[test]
fn prev_match_navigation() {
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
            enter(),  // finds "foo 1" (line 0)
            key('N'), // previous (backward): wraps to "foo 3" (line 4)
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
:
-----
[EVENT]:char:N
{rev}{line}{b}f{/rev}{/line}{/b}{line}{b}oo{/line}{/b} 2
baz
{rev}{b}foo{/rev}{/b} 3
:
-----
";
    assert_eq!(screen.out(), want);
}

// No match: position stays the same.
#[test]
fn no_match_stays_in_place() {
    let content = "\
line 1
line 2
line 3
line 4
";
    let screen = run_test_screen(TestCase {
        screen_width: 20,
        screen_height: 4,
        content,
        events: vec![key('/'), key('z'), key('z'), key('z'), enter()],
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
[EVENT]:char:z
line 1
line 2
line 3
/z█
-----
[EVENT]:char:z
line 1
line 2
line 3
/zz█
-----
[EVENT]:char:z
line 1
line 2
line 3
/zzz█
-----
[EVENT]:enter
line 1
line 2
line 3
:
-----
";
    assert_eq!(screen.out(), want);
}

// Wrap around: search wraps from end to beginning.
#[test]
fn search_wraps_around() {
    let content = "\
target here
line 2
line 3
line 4
line 5
";
    let screen = run_test_screen(TestCase {
        screen_width: 20,
        screen_height: 4,
        content,
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
        ..Default::default()
    });
    let want = "\
target here
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
{rev}{b}t{/rev}{/b}arge{rev}{line}{b}t{/rev}{/line}{/b} here
line 2
line 3
/t█
-----
[EVENT]:char:a
{rev}{b}ta{/rev}{/b}rget here
line 2
line 3
/ta█
-----
[EVENT]:char:r
{rev}{b}tar{/rev}{/b}get here
line 2
line 3
/tar█
-----
[EVENT]:char:g
{rev}{b}targ{/rev}{/b}et here
line 2
line 3
/targ█
-----
[EVENT]:char:e
{rev}{b}targe{/rev}{/b}t here
line 2
line 3
/targe█
-----
[EVENT]:char:t
{rev}{b}target{/rev}{/b} here
line 2
line 3
/target█
-----
[EVENT]:enter
{rev}{b}target{/rev}{/b} here
line 2
line 3
:
-----
";
    assert_eq!(screen.out(), want);
}

// Highlighting with ANSI escape sequences: match spans across escape sequences.
#[test]
fn highlight_with_ansi_escapes() {
    let content = "\
line 1
This is \x1b[1mCargo\x1b[0m.toml
line 3
line 4
";
    let screen = run_test_screen(TestCase {
        screen_width: 30,
        screen_height: 4,
        content,
        events: vec![
            key('/'),
            key('C'),
            key('a'),
            key('r'),
            key('g'),
            key('o'),
            enter(),
        ],
        ..Default::default()
    });
    // The bold start (\x1b[1m) precedes "Cargo" in raw text and the
    // reset (\x1b[0m) follows it. The match end position in raw text
    // falls after the reset, so the reset appears inside the highlighted span.
    let want = "\
line 1
This is {b}Cargo{reset}.toml
line 3
:
-----
[EVENT]:char:/
line 1
This is {b}Cargo{reset}.toml
line 3
/█
-----
[EVENT]:char:C
This is {b}{rev}{b}C{/rev}{/b}argo{reset}.toml
line 3
line 4
/C█
-----
[EVENT]:char:a
This is {b}{rev}{b}Ca{/rev}{/b}rgo{reset}.toml
line 3
line 4
/Ca█
-----
[EVENT]:char:r
This is {b}{rev}{b}Car{/rev}{/b}go{reset}.toml
line 3
line 4
/Car█
-----
[EVENT]:char:g
This is {b}{rev}{b}Carg{/rev}{/b}o{reset}.toml
line 3
line 4
/Carg█
-----
[EVENT]:char:o
This is {b}{rev}{b}Cargo{reset}{/rev}{/b}.toml
line 3
line 4
/Cargo█
-----
[EVENT]:enter
This is {b}{rev}{b}Cargo{reset}{/rev}{/b}.toml
line 3
line 4
:
-----
";
    assert_eq!(screen.out(), want);
}

// Re-searching with a different keyword replaces the highlights.
#[test]
fn re_search_with_different_keyword() {
    let content = "\
line 1
line 2
line 3
line 4
line 5
line 6
";
    let out = run_test_screen(TestCase {
        screen_width: 20,
        screen_height: 4,
        content,
        events: vec![
            // Search by "li".
            key('/'),
            key('l'),
            key('i'),
            enter(),
            // Navigate down.
            key('j'),
            key('j'),
            // Search by "ne" (replaces "li").
            key('/'),
            key('n'),
            key('e'),
            enter(),
            // Navigate and verify new highlights.
            key('j'),
            key('k'),
        ],
        ..Default::default()
    })
    .out();
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
[EVENT]:char:l
{rev}{b}l{/rev}{/b}ine 1
{rev}{line}{b}l{/rev}{/line}{/b}ine 2
{rev}{line}{b}l{/rev}{/line}{/b}ine 3
/l█
-----
[EVENT]:char:i
{rev}{b}li{/rev}{/b}ne 1
{rev}{line}{b}l{/rev}{/line}{/b}{line}{b}i{/line}{/b}ne 2
{rev}{line}{b}l{/rev}{/line}{/b}{line}{b}i{/line}{/b}ne 3
/li█
-----
[EVENT]:enter
{rev}{b}li{/rev}{/b}ne 1
{rev}{line}{b}l{/rev}{/line}{/b}{line}{b}i{/line}{/b}ne 2
{rev}{line}{b}l{/rev}{/line}{/b}{line}{b}i{/line}{/b}ne 3
:
-----
[EVENT]:char:j
{rev}{line}{b}l{/rev}{/line}{/b}{line}{b}i{/line}{/b}ne 2
{rev}{line}{b}l{/rev}{/line}{/b}{line}{b}i{/line}{/b}ne 3
{rev}{line}{b}l{/rev}{/line}{/b}{line}{b}i{/line}{/b}ne 4
:
-----
[EVENT]:char:j
{rev}{line}{b}l{/rev}{/line}{/b}{line}{b}i{/line}{/b}ne 3
{rev}{line}{b}l{/rev}{/line}{/b}{line}{b}i{/line}{/b}ne 4
{rev}{line}{b}l{/rev}{/line}{/b}{line}{b}i{/line}{/b}ne 5
:
-----
[EVENT]:char:/
{rev}{line}{b}l{/rev}{/line}{/b}{line}{b}i{/line}{/b}ne 3
{rev}{line}{b}l{/rev}{/line}{/b}{line}{b}i{/line}{/b}ne 4
{rev}{line}{b}l{/rev}{/line}{/b}{line}{b}i{/line}{/b}ne 5
/█
-----
[EVENT]:char:n
li{rev}{b}n{/rev}{/b}e 3
li{rev}{line}{b}n{/rev}{/line}{/b}e 4
li{rev}{line}{b}n{/rev}{/line}{/b}e 5
/n█
-----
[EVENT]:char:e
li{rev}{b}ne{/rev}{/b} 3
li{rev}{line}{b}n{/rev}{/line}{/b}{line}{b}e{/line}{/b} 4
li{rev}{line}{b}n{/rev}{/line}{/b}{line}{b}e{/line}{/b} 5
/ne█
-----
[EVENT]:enter
li{rev}{b}ne{/rev}{/b} 3
li{rev}{line}{b}n{/rev}{/line}{/b}{line}{b}e{/line}{/b} 4
li{rev}{line}{b}n{/rev}{/line}{/b}{line}{b}e{/line}{/b} 5
:
-----
[EVENT]:char:j
li{rev}{line}{b}n{/rev}{/line}{/b}{line}{b}e{/line}{/b} 4
li{rev}{line}{b}n{/rev}{/line}{/b}{line}{b}e{/line}{/b} 5
li{rev}{line}{b}n{/rev}{/line}{/b}{line}{b}e{/line}{/b} 6
:
-----
[EVENT]:char:k
li{rev}{b}ne{/rev}{/b} 3
li{rev}{line}{b}n{/rev}{/line}{/b}{line}{b}e{/line}{/b} 4
li{rev}{line}{b}n{/rev}{/line}{/b}{line}{b}e{/line}{/b} 5
:
-----
";
    assert_eq!(out, want);
}

// Multiple matches on the same line: the current match (first) uses reverse + bold,
// other matches on the same line use underline + bold with their first character
// also reversed.
#[test]
fn multiple_matches_same_line() {
    let content = "\
foo bar foo baz foo
line 2
line 3
line 4
";
    let screen = run_test_screen(TestCase {
        screen_width: 30,
        screen_height: 4,
        content,
        events: vec![key('/'), key('f'), key('o'), key('o'), enter()],
        ..Default::default()
    });
    let want = "\
foo bar foo baz foo
line 2
line 3
:
-----
[EVENT]:char:/
foo bar foo baz foo
line 2
line 3
/█
-----
[EVENT]:char:f
{rev}{b}f{/rev}{/b}oo bar {rev}{line}{b}f{/rev}{/line}{/b}oo baz {rev}{line}{b}f{/rev}{/line}{/b}oo
line 2
line 3
/f█
-----
[EVENT]:char:o
{rev}{b}fo{/rev}{/b}o bar {rev}{line}{b}f{/rev}{/line}{/b}{line}{b}o{/line}{/b}o baz {rev}{line}{b}f{/rev}{/line}{/b}{line}{b}o{/line}{/b}o
line 2
line 3
/fo█
-----
[EVENT]:char:o
{rev}{b}foo{/rev}{/b} bar {rev}{line}{b}f{/rev}{/line}{/b}{line}{b}oo{/line}{/b} baz {rev}{line}{b}f{/rev}{/line}{/b}{line}{b}oo{/line}{/b}
line 2
line 3
/foo█
-----
[EVENT]:enter
{rev}{b}foo{/rev}{/b} bar {rev}{line}{b}f{/rev}{/line}{/b}{line}{b}oo{/line}{/b} baz {rev}{line}{b}f{/rev}{/line}{/b}{line}{b}oo{/line}{/b}
line 2
line 3
:
-----
";
    assert_eq!(screen.out(), want);
}

// When jumping to a match near the end of the document, the status line
// must remain at the bottom of the screen. Previously, jump_to would
// shrink the rows array when there weren't enough lines after the match,
// causing the status line to move up.
#[test]
fn status_line_stays_at_bottom_when_jumping_near_end() {
    let content = "\
line 1
line 2
line 3
line 4
target foo here
line 6
";
    let screen = run_test_screen(TestCase {
        screen_width: 20,
        screen_height: 5,
        content,
        // Search for "target", then press n to cycle. First match is at
        // line 4 which is near the end — only 2 lines of content remain
        // (lines 4 and 5), but the screen has 4 content rows.
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
    // The matched line should be visible and the status line must be on
    // the last row (row 4), not immediately after the last content line.
    let want = "\
line 1
line 2
line 3
line 4
:
-----
[EVENT]:char:/
line 1
line 2
line 3
line 4
/█
-----
[EVENT]:char:t
line 3
line 4
{rev}{b}t{/rev}{/b}arge{rev}{line}{b}t{/rev}{/line}{/b} foo here
line 6
/t█
-----
[EVENT]:char:a
line 3
line 4
{rev}{b}ta{/rev}{/b}rget foo here
line 6
/ta█
-----
[EVENT]:char:r
line 3
line 4
{rev}{b}tar{/rev}{/b}get foo here
line 6
/tar█
-----
[EVENT]:char:g
line 3
line 4
{rev}{b}targ{/rev}{/b}et foo here
line 6
/targ█
-----
[EVENT]:char:e
line 3
line 4
{rev}{b}targe{/rev}{/b}t foo here
line 6
/targe█
-----
[EVENT]:char:t
line 3
line 4
{rev}{b}target{/rev}{/b} foo here
line 6
/target█
-----
[EVENT]:enter
line 3
line 4
{rev}{b}target{/rev}{/b} foo here
line 6
:
-----
";
    assert_eq!(screen.out(), want);
}

// When search jumps to a match near the end, downward scrolling should be
// blocked at the bottom, but upward scrolling should work normally.
#[test]
fn near_end_blocks_scroll_down_at_bottom() {
    let content = "\
line 1
line 2
line 3
line 4
line 5
line 6
line 7
line 8
line 9
line 10
";
    let out = run_test_screen(TestCase {
        screen_width: 10,
        screen_height: 5,
        content,
        events: vec![
            key('/'),
            key('9'),
            enter(),
            // Already at bottom — j should be blocked.
            key('j'),
            // Can scroll up.
            key('k'),
            // Scroll down works now.
            key('j'),
        ],
        ..Default::default()
    })
    .out();
    let want = "\
line 1
line 2
line 3
line 4
:
-----
[EVENT]:char:/
line 1
line 2
line 3
line 4
/█
-----
[EVENT]:char:9
line 7
line 8
line {rev}{b}9{/rev}{/b}
line 10
/9█
-----
[EVENT]:enter
line 7
line 8
line {rev}{b}9{/rev}{/b}
line 10
:
-----
[EVENT]:char:j
[EVENT]:char:k
line 6
line 7
line 8
line {rev}{b}9{/rev}{/b}
:
-----
[EVENT]:char:j
line 7
line 8
line {rev}{b}9{/rev}{/b}
line 10
:
-----
";
    assert_eq!(out, want);
}

// Verify final state after scrolling past a near-end match.
#[test]
fn near_end_scroll_up_then_down() {
    let content = "\
line 1
line 2
line 3
line 4
line 5
line 6
line 7
line 8
line 9
line 10
";
    let screen = run_test_screen(TestCase {
        screen_width: 10,
        screen_height: 5,
        content,
        events: vec![
            key('/'),
            key('9'),
            enter(),
            // Scroll up twice, then back down.
            key('k'),
            key('k'),
            key('j'),
        ],
        ..Default::default()
    });
    let want = "\
line 1
line 2
line 3
line 4
:
-----
[EVENT]:char:/
line 1
line 2
line 3
line 4
/█
-----
[EVENT]:char:9
line 7
line 8
line {rev}{b}9{/rev}{/b}
line 10
/9█
-----
[EVENT]:enter
line 7
line 8
line {rev}{b}9{/rev}{/b}
line 10
:
-----
[EVENT]:char:k
line 6
line 7
line 8
line {rev}{b}9{/rev}{/b}
:
-----
[EVENT]:char:k
line 5
line 6
line 7
line 8
:
-----
[EVENT]:char:j
line 6
line 7
line 8
line {rev}{b}9{/rev}{/b}
:
-----
";
    assert_eq!(screen.out(), want);
}
