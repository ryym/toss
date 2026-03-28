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
line 3
target {reverse}foo{/reverse} here
line 5
:
";
    assert_eq!(screen.last_snapshot(), want);
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
{reverse}t{/reverse}op line
line 2
line 3
?t█
-----
[EVENT]:char:o
{reverse}to{/reverse}p line
line 2
line 3
?to█
-----
[EVENT]:char:p
{reverse}top{/reverse} line
line 2
line 3
?top█
-----
[EVENT]:enter
{reverse}top{/reverse} line
line 2
line 3
:
-----
";
    assert_eq!(out, want);
}

// n key: jump to next match in search direction.
// Current match uses reverse, other matches use dim reverse.
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
{reverse}foo{/reverse} 2
baz
{dim}{reverse}foo{/reverse}{/dim} 3
:
";
    assert_eq!(screen.last_snapshot(), want);
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
{dim}{reverse}foo{/reverse}{/dim} 2
baz
{reverse}foo{/reverse} 3
:
";
    assert_eq!(screen.last_snapshot(), want);
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
";
    assert_eq!(screen.last_snapshot(), want);
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
{reverse}target{/reverse} here
line 2
line 3
:
";
    assert_eq!(screen.last_snapshot(), want);
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
This is {bold}{reverse}Cargo{reset}{/reverse}.toml
line 3
line 4
:
";
    assert_eq!(screen.last_snapshot(), want);
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
{reverse}l{/reverse}ine 1
{dim}{reverse}l{/reverse}{/dim}ine 2
{dim}{reverse}l{/reverse}{/dim}ine 3
/l█
-----
[EVENT]:char:i
{reverse}li{/reverse}ne 1
{dim}{reverse}li{/reverse}{/dim}ne 2
{dim}{reverse}li{/reverse}{/dim}ne 3
/li█
-----
[EVENT]:enter
{reverse}li{/reverse}ne 1
{dim}{reverse}li{/reverse}{/dim}ne 2
{dim}{reverse}li{/reverse}{/dim}ne 3
:
-----
[EVENT]:char:j
{dim}{reverse}li{/reverse}{/dim}ne 2
{dim}{reverse}li{/reverse}{/dim}ne 3
{dim}{reverse}li{/reverse}{/dim}ne 4
:
-----
[EVENT]:char:j
{dim}{reverse}li{/reverse}{/dim}ne 3
{dim}{reverse}li{/reverse}{/dim}ne 4
{dim}{reverse}li{/reverse}{/dim}ne 5
:
-----
[EVENT]:char:/
{dim}{reverse}li{/reverse}{/dim}ne 3
{dim}{reverse}li{/reverse}{/dim}ne 4
{dim}{reverse}li{/reverse}{/dim}ne 5
/█
-----
[EVENT]:char:n
li{reverse}n{/reverse}e 3
li{dim}{reverse}n{/reverse}{/dim}e 4
li{dim}{reverse}n{/reverse}{/dim}e 5
/n█
-----
[EVENT]:char:e
li{reverse}ne{/reverse} 3
li{dim}{reverse}ne{/reverse}{/dim} 4
li{dim}{reverse}ne{/reverse}{/dim} 5
/ne█
-----
[EVENT]:enter
li{reverse}ne{/reverse} 3
li{dim}{reverse}ne{/reverse}{/dim} 4
li{dim}{reverse}ne{/reverse}{/dim} 5
:
-----
[EVENT]:char:j
li{dim}{reverse}ne{/reverse}{/dim} 4
li{dim}{reverse}ne{/reverse}{/dim} 5
li{dim}{reverse}ne{/reverse}{/dim} 6
:
-----
[EVENT]:char:k
li{reverse}ne{/reverse} 3
li{dim}{reverse}ne{/reverse}{/dim} 4
li{dim}{reverse}ne{/reverse}{/dim} 5
:
-----
";
    assert_eq!(out, want);
}

// Multiple matches on the same line: the current match (first) uses reverse,
// other matches on the same line use dim reverse.
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
{reverse}foo{/reverse} bar {dim}{reverse}foo{/reverse}{/dim} baz {dim}{reverse}foo{/reverse}{/dim}
line 2
line 3
:
";
    assert_eq!(screen.last_snapshot(), want);
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
line 3
line 4
{reverse}target{/reverse} foo here
line 6
:
";
    assert_eq!(screen.last_snapshot(), want);
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
line {reverse}9{/reverse}
line 10
/9█
-----
[EVENT]:enter
line 7
line 8
line {reverse}9{/reverse}
line 10
:
-----
[EVENT]:char:j
[EVENT]:char:k
line 6
line 7
line 8
line {reverse}9{/reverse}
:
-----
[EVENT]:char:j
line 7
line 8
line {reverse}9{/reverse}
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
line 6
line 7
line 8
line {reverse}9{/reverse}
:
";
    assert_eq!(screen.last_snapshot(), want);
}
