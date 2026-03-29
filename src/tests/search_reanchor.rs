use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use pretty_assertions::assert_eq;

use super::{TestCase, key, run_test_screen};

fn enter() -> Event {
    Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
}

fn search_foo() -> Vec<Event> {
    vec![key('/'), key('f'), key('o'), key('o'), enter()]
}

// When the cursor is outside the viewport and visible matches exist,
// pressing n re-anchors the cursor to the first visible match without scrolling.
// The second n then jumps forward from the re-anchored position.
#[test]
fn reanchor_to_first_visible_match() {
    let content = "\
foo 1
line 2
line 3
line 4
line 5
line 6
foo 7
line 8
foo 9
line 10
";
    let mut events = search_foo();
    // Jump to end — cursor ("foo 1") goes off-screen.
    events.push(key('G'));
    // First n: re-anchors to "foo 9" (first visible match). No scroll.
    // Second n: jumps from "foo 9" forward, wrapping to "foo 1".
    events.push(key('n'));
    events.push(key('n'));

    let screen = run_test_screen(TestCase {
        screen_width: 20,
        screen_height: 4,
        content,
        events,
        ..Default::default()
    });
    // Final state: jumped to "foo 1" — proves the second n started from "foo 9".
    let want = "\
{reverse}foo{/reverse} 1
line 2
line 3
:
";
    assert_eq!(screen.last_snapshot(), want);
}

// When the cursor is outside the viewport and no matches are visible,
// pressing n searches forward from the viewport's top line.
#[test]
fn reanchor_searches_from_viewport_top() {
    let content = "\
foo 1
line 2
line 3
line 4
line 5
line 6
line 7
foo 8
";
    let mut events = search_foo();
    // Scroll down so cursor ("foo 1") is off-screen and no matches visible.
    events.extend([key('j'), key('j'), key('j')]);
    // n: searches forward from viewport top, finds "foo 8".
    events.push(key('n'));

    let screen = run_test_screen(TestCase {
        screen_width: 20,
        screen_height: 4,
        content,
        events,
        ..Default::default()
    });
    let want = "\
line 6
line 7
{reverse}foo{/reverse} 8
:
";
    assert_eq!(screen.last_snapshot(), want);
}

// When the cursor is still within the viewport, n jumps normally.
#[test]
fn no_reanchor_when_cursor_visible() {
    let content = "\
line 1
foo 2
line 3
foo 4
line 5
foo 6
";
    let mut events = search_foo();
    // Scroll down 1 line. Cursor ("foo 2", line 1) is still visible.
    events.push(key('j'));
    // n: cursor visible, so normal jump to "foo 4".
    events.push(key('n'));

    let screen = run_test_screen(TestCase {
        screen_width: 20,
        screen_height: 4,
        content,
        events,
        ..Default::default()
    });
    let want = "\
line 3
{reverse}foo{/reverse} 4
line 5
:
";
    assert_eq!(screen.last_snapshot(), want);
}

// Reverse search (N) also re-anchors when cursor is off-screen.
#[test]
fn reanchor_reverse_direction() {
    let content = "\
line 1
line 2
line 3
foo 4
line 5
foo 6
line 7
foo 8
";
    let mut events = search_foo();
    // Jump to end — cursor ("foo 4") goes off-screen.
    events.push(key('G'));
    // N (reverse): re-anchors to first visible match.
    // Second N: jumps backward from the re-anchored position.
    events.push(key('N'));
    events.push(key('N'));

    let screen = run_test_screen(TestCase {
        screen_width: 20,
        screen_height: 4,
        content,
        events,
        ..Default::default()
    });
    // Re-anchored to "foo 6" (first visible), then N backward to "foo 4".
    let want = "\
{reverse}foo{/reverse} 4
line 5
{dim}{reverse}foo{/reverse}{/dim} 6
:
";
    assert_eq!(screen.last_snapshot(), want);
}

// Verify the intermediate re-anchor state: after G + n, the cursor moves
// to the first visible match WITHOUT scrolling (only highlight changes).
#[test]
fn reanchor_does_not_scroll() {
    let content = "\
foo 1
line 2
line 3
line 4
foo 5
line 6
foo 7
";
    let mut events = search_foo();
    events.push(key('G'));
    events.push(key('n'));

    let out = run_test_screen(TestCase {
        screen_width: 20,
        screen_height: 4,
        content,
        events,
        ..Default::default()
    })
    .out();

    // Extract just the last two snapshots (after G and after n).
    let snapshots: Vec<&str> = out.split("-----\n").collect();
    let len = snapshots.len();

    // After G: viewport at end, matches dimmed (not current).
    let after_g = snapshots[len - 3]
        .trim_start_matches(|c: char| c != '\n')
        .trim_start();
    assert_eq!(
        after_g,
        "\
{dim}{reverse}foo{/reverse}{/dim} 5
line 6
{dim}{reverse}foo{/reverse}{/dim} 7
:
"
    );

    // After n: same viewport, but "foo 5" is now current (re-anchored).
    let after_n = snapshots[len - 2]
        .trim_start_matches(|c: char| c != '\n')
        .trim_start();
    assert_eq!(
        after_n,
        "\
{reverse}foo{/reverse} 5
line 6
{dim}{reverse}foo{/reverse}{/dim} 7
:
"
    );
}
