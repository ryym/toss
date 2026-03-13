use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use pretty_assertions::assert_eq;

use super::{TestCase, key, run_test};

fn enter() -> Event {
    Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
}

// Forward search: /foo + Enter jumps to the line containing "foo"
// and highlights the match with reverse video.
#[test]
fn forward_search_jumps_to_match() {
    let out = run_test(TestCase {
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

    // After search, the screen should show lines starting from "target foo here"
    // with "foo" highlighted using reverse video.
    assert!(out.contains("target {reverse}foo{/reverse} here"));
}

// Backward search: ?foo + Enter searches backwards.
#[test]
fn backward_search_jumps_to_match() {
    // Start at the bottom with G, then search backward for "top".
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

    // Should jump back to "top line" with "top" highlighted.
    assert!(out.contains("{reverse}top{/reverse} line"));
}

// n key: jump to next match in search direction.
#[test]
fn next_match_navigation() {
    let out = run_test(TestCase {
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

    // After n, screen should show "foo 2" at top.
    // Find the last snapshot (after the 'n' key press)
    let last_separator = out.rfind("-----").unwrap();
    let second_last = out[..last_separator].rfind("-----").unwrap();
    let last_snapshot = &out[second_last + 6..last_separator];

    // The last snapshot should contain "foo 2" with highlight
    assert!(
        last_snapshot.contains("{reverse}foo{/reverse} 2"),
        "Expected highlighted 'foo 2' in last snapshot, got:\n{last_snapshot}"
    );
}

// N key: jump to previous match (reverse direction).
#[test]
fn prev_match_navigation() {
    let out = run_test(TestCase {
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

    let last_separator = out.rfind("-----").unwrap();
    let second_last = out[..last_separator].rfind("-----").unwrap();
    let last_snapshot = &out[second_last + 6..last_separator];

    assert!(
        last_snapshot.contains("{reverse}foo{/reverse} 3"),
        "Expected highlighted 'foo 3' in last snapshot, got:\n{last_snapshot}"
    );
}

// No match: position stays the same.
#[test]
fn no_match_stays_in_place() {
    let out = run_test(TestCase {
        screen_width: 20,
        screen_height: 4,
        content: "\
line 1
line 2
line 3
line 4",
        events: vec![key('/'), key('z'), key('z'), key('z'), enter()],
    });

    // After Enter with no match, the screen should still show the original content.
    let last_separator = out.rfind("-----").unwrap();
    let second_last = out[..last_separator].rfind("-----").unwrap();
    let last_snapshot = &out[second_last + 6..last_separator];

    assert!(
        last_snapshot.contains("line 1"),
        "Expected 'line 1' to still be visible, got:\n{last_snapshot}"
    );
}

// Wrap around: search wraps from end to beginning.
#[test]
fn search_wraps_around() {
    let out = run_test(TestCase {
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

    let last_separator = out.rfind("-----").unwrap();
    let second_last = out[..last_separator].rfind("-----").unwrap();
    let last_snapshot = &out[second_last + 6..last_separator];

    assert!(
        last_snapshot.contains("{reverse}target{/reverse} here"),
        "Expected highlighted 'target' after wrap-around, got:\n{last_snapshot}"
    );
}

// Highlighting with ANSI escape sequences: match spans across escape sequences.
#[test]
fn highlight_with_ansi_escapes() {
    let out = run_test(TestCase {
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

    // The match "Cargo" spans bold escape sequences.
    // In the rendered output, "Cargo" should be highlighted.
    let last_separator = out.rfind("-----").unwrap();
    let second_last = out[..last_separator].rfind("-----").unwrap();
    let last_snapshot = &out[second_last + 6..last_separator];

    // "Cargo" has bold on and reset escape sequences around it in the raw text.
    // The highlight wraps the match including the reset sequence that falls
    // between the last matched byte and the next character in raw text.
    assert!(
        last_snapshot.contains("{bold}{reverse}Cargo{reset}{/reverse}.toml"),
        "Expected highlighted 'Cargo' with ANSI escapes preserved, got:\n{last_snapshot}"
    );
}

// Multiple matches on the same line are all highlighted.
#[test]
fn multiple_matches_same_line() {
    let out = run_test(TestCase {
        screen_width: 30,
        screen_height: 4,
        content: "\
foo bar foo baz foo
line 2
line 3
line 4",
        events: vec![key('/'), key('f'), key('o'), key('o'), enter()],
    });

    let last_separator = out.rfind("-----").unwrap();
    let second_last = out[..last_separator].rfind("-----").unwrap();
    let last_snapshot = &out[second_last + 6..last_separator];

    // Count occurrences of highlighted "foo"
    let highlight_count = last_snapshot.matches("{reverse}foo{/reverse}").count();
    assert_eq!(
        highlight_count, 3,
        "Expected 3 highlighted 'foo' matches, got {highlight_count} in:\n{last_snapshot}"
    );
}
