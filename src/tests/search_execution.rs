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
target {reverse}{bold}f{/reverse}{/bold}oo here
line 5
/f█
-----
[EVENT]:char:o
line 3
target {reverse}{bold}fo{/reverse}{/bold}o here
line 5
/fo█
-----
[EVENT]:char:o
line 3
target {reverse}{bold}foo{/reverse}{/bold} here
line 5
/foo█
-----
[EVENT]:enter
line 3
target {reverse}{bold}foo{/reverse}{/bold} here
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
{reverse}{bold}t{/reverse}{/bold}op line
line 2
line 3
?t█
-----
[EVENT]:char:o
{reverse}{bold}to{/reverse}{/bold}p line
line 2
line 3
?to█
-----
[EVENT]:char:p
{reverse}{bold}top{/reverse}{/bold} line
line 2
line 3
?top█
-----
[EVENT]:enter
{reverse}{bold}top{/reverse}{/bold} line
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
[EVENT]:char:N
{reverse}{underline}{bold}f{/reverse}{/underline}{/bold}{underline}{bold}oo{/underline}{/bold} 2
baz
{reverse}{bold}foo{/reverse}{/bold} 3
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
{reverse}{bold}t{/reverse}{/bold}arge{reverse}{underline}{bold}t{/reverse}{/underline}{/bold} here
line 2
line 3
/t█
-----
[EVENT]:char:a
{reverse}{bold}ta{/reverse}{/bold}rget here
line 2
line 3
/ta█
-----
[EVENT]:char:r
{reverse}{bold}tar{/reverse}{/bold}get here
line 2
line 3
/tar█
-----
[EVENT]:char:g
{reverse}{bold}targ{/reverse}{/bold}et here
line 2
line 3
/targ█
-----
[EVENT]:char:e
{reverse}{bold}targe{/reverse}{/bold}t here
line 2
line 3
/targe█
-----
[EVENT]:char:t
{reverse}{bold}target{/reverse}{/bold} here
line 2
line 3
/target█
-----
[EVENT]:enter
{reverse}{bold}target{/reverse}{/bold} here
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
This is {bold}Cargo{reset}.toml
line 3
:
-----
[EVENT]:char:/
line 1
This is {bold}Cargo{reset}.toml
line 3
/█
-----
[EVENT]:char:C
This is {bold}{reverse}{bold}C{/reverse}{/bold}argo{reset}.toml
line 3
line 4
/C█
-----
[EVENT]:char:a
This is {bold}{reverse}{bold}Ca{/reverse}{/bold}rgo{reset}.toml
line 3
line 4
/Ca█
-----
[EVENT]:char:r
This is {bold}{reverse}{bold}Car{/reverse}{/bold}go{reset}.toml
line 3
line 4
/Car█
-----
[EVENT]:char:g
This is {bold}{reverse}{bold}Carg{/reverse}{/bold}o{reset}.toml
line 3
line 4
/Carg█
-----
[EVENT]:char:o
This is {bold}{reverse}{bold}Cargo{reset}{/reverse}{/bold}.toml
line 3
line 4
/Cargo█
-----
[EVENT]:enter
This is {bold}{reverse}{bold}Cargo{reset}{/reverse}{/bold}.toml
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
{reverse}{bold}l{/reverse}{/bold}ine 1
{reverse}{underline}{bold}l{/reverse}{/underline}{/bold}ine 2
{reverse}{underline}{bold}l{/reverse}{/underline}{/bold}ine 3
/l█
-----
[EVENT]:char:i
{reverse}{bold}li{/reverse}{/bold}ne 1
{reverse}{underline}{bold}l{/reverse}{/underline}{/bold}{underline}{bold}i{/underline}{/bold}ne 2
{reverse}{underline}{bold}l{/reverse}{/underline}{/bold}{underline}{bold}i{/underline}{/bold}ne 3
/li█
-----
[EVENT]:enter
{reverse}{bold}li{/reverse}{/bold}ne 1
{reverse}{underline}{bold}l{/reverse}{/underline}{/bold}{underline}{bold}i{/underline}{/bold}ne 2
{reverse}{underline}{bold}l{/reverse}{/underline}{/bold}{underline}{bold}i{/underline}{/bold}ne 3
:
-----
[EVENT]:char:j
{reverse}{underline}{bold}l{/reverse}{/underline}{/bold}{underline}{bold}i{/underline}{/bold}ne 2
{reverse}{underline}{bold}l{/reverse}{/underline}{/bold}{underline}{bold}i{/underline}{/bold}ne 3
{reverse}{underline}{bold}l{/reverse}{/underline}{/bold}{underline}{bold}i{/underline}{/bold}ne 4
:
-----
[EVENT]:char:j
{reverse}{underline}{bold}l{/reverse}{/underline}{/bold}{underline}{bold}i{/underline}{/bold}ne 3
{reverse}{underline}{bold}l{/reverse}{/underline}{/bold}{underline}{bold}i{/underline}{/bold}ne 4
{reverse}{underline}{bold}l{/reverse}{/underline}{/bold}{underline}{bold}i{/underline}{/bold}ne 5
:
-----
[EVENT]:char:/
{reverse}{underline}{bold}l{/reverse}{/underline}{/bold}{underline}{bold}i{/underline}{/bold}ne 3
{reverse}{underline}{bold}l{/reverse}{/underline}{/bold}{underline}{bold}i{/underline}{/bold}ne 4
{reverse}{underline}{bold}l{/reverse}{/underline}{/bold}{underline}{bold}i{/underline}{/bold}ne 5
/█
-----
[EVENT]:char:n
li{reverse}{bold}n{/reverse}{/bold}e 3
li{reverse}{underline}{bold}n{/reverse}{/underline}{/bold}e 4
li{reverse}{underline}{bold}n{/reverse}{/underline}{/bold}e 5
/n█
-----
[EVENT]:char:e
li{reverse}{bold}ne{/reverse}{/bold} 3
li{reverse}{underline}{bold}n{/reverse}{/underline}{/bold}{underline}{bold}e{/underline}{/bold} 4
li{reverse}{underline}{bold}n{/reverse}{/underline}{/bold}{underline}{bold}e{/underline}{/bold} 5
/ne█
-----
[EVENT]:enter
li{reverse}{bold}ne{/reverse}{/bold} 3
li{reverse}{underline}{bold}n{/reverse}{/underline}{/bold}{underline}{bold}e{/underline}{/bold} 4
li{reverse}{underline}{bold}n{/reverse}{/underline}{/bold}{underline}{bold}e{/underline}{/bold} 5
:
-----
[EVENT]:char:j
li{reverse}{underline}{bold}n{/reverse}{/underline}{/bold}{underline}{bold}e{/underline}{/bold} 4
li{reverse}{underline}{bold}n{/reverse}{/underline}{/bold}{underline}{bold}e{/underline}{/bold} 5
li{reverse}{underline}{bold}n{/reverse}{/underline}{/bold}{underline}{bold}e{/underline}{/bold} 6
:
-----
[EVENT]:char:k
li{reverse}{bold}ne{/reverse}{/bold} 3
li{reverse}{underline}{bold}n{/reverse}{/underline}{/bold}{underline}{bold}e{/underline}{/bold} 4
li{reverse}{underline}{bold}n{/reverse}{/underline}{/bold}{underline}{bold}e{/underline}{/bold} 5
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
{reverse}{bold}f{/reverse}{/bold}oo bar {reverse}{underline}{bold}f{/reverse}{/underline}{/bold}oo baz {reverse}{underline}{bold}f{/reverse}{/underline}{/bold}oo
line 2
line 3
/f█
-----
[EVENT]:char:o
{reverse}{bold}fo{/reverse}{/bold}o bar {reverse}{underline}{bold}f{/reverse}{/underline}{/bold}{underline}{bold}o{/underline}{/bold}o baz {reverse}{underline}{bold}f{/reverse}{/underline}{/bold}{underline}{bold}o{/underline}{/bold}o
line 2
line 3
/fo█
-----
[EVENT]:char:o
{reverse}{bold}foo{/reverse}{/bold} bar {reverse}{underline}{bold}f{/reverse}{/underline}{/bold}{underline}{bold}oo{/underline}{/bold} baz {reverse}{underline}{bold}f{/reverse}{/underline}{/bold}{underline}{bold}oo{/underline}{/bold}
line 2
line 3
/foo█
-----
[EVENT]:enter
{reverse}{bold}foo{/reverse}{/bold} bar {reverse}{underline}{bold}f{/reverse}{/underline}{/bold}{underline}{bold}oo{/underline}{/bold} baz {reverse}{underline}{bold}f{/reverse}{/underline}{/bold}{underline}{bold}oo{/underline}{/bold}
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
{reverse}{bold}t{/reverse}{/bold}arge{reverse}{underline}{bold}t{/reverse}{/underline}{/bold} foo here
line 6
/t█
-----
[EVENT]:char:a
line 3
line 4
{reverse}{bold}ta{/reverse}{/bold}rget foo here
line 6
/ta█
-----
[EVENT]:char:r
line 3
line 4
{reverse}{bold}tar{/reverse}{/bold}get foo here
line 6
/tar█
-----
[EVENT]:char:g
line 3
line 4
{reverse}{bold}targ{/reverse}{/bold}et foo here
line 6
/targ█
-----
[EVENT]:char:e
line 3
line 4
{reverse}{bold}targe{/reverse}{/bold}t foo here
line 6
/targe█
-----
[EVENT]:char:t
line 3
line 4
{reverse}{bold}target{/reverse}{/bold} foo here
line 6
/target█
-----
[EVENT]:enter
line 3
line 4
{reverse}{bold}target{/reverse}{/bold} foo here
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
line {reverse}{bold}9{/reverse}{/bold}
line 10
/9█
-----
[EVENT]:enter
line 7
line 8
line {reverse}{bold}9{/reverse}{/bold}
line 10
:
-----
[EVENT]:char:j
[EVENT]:char:k
line 6
line 7
line 8
line {reverse}{bold}9{/reverse}{/bold}
:
-----
[EVENT]:char:j
line 7
line 8
line {reverse}{bold}9{/reverse}{/bold}
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
line {reverse}{bold}9{/reverse}{/bold}
line 10
/9█
-----
[EVENT]:enter
line 7
line 8
line {reverse}{bold}9{/reverse}{/bold}
line 10
:
-----
[EVENT]:char:k
line 6
line 7
line 8
line {reverse}{bold}9{/reverse}{/bold}
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
line {reverse}{bold}9{/reverse}{/bold}
:
-----
";
    assert_eq!(screen.out(), want);
}
