use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use pretty_assertions::assert_eq;

use super::{TestCase, key, run_test_screen};

fn enter() -> Event {
    Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
}

// When jumping to a match near the end of the document, the status line
// must remain at the bottom of the screen. Previously, jump_to would
// shrink the rows array when there weren't enough lines after the match,
// causing the status line to move up.
#[test]
fn status_line_stays_at_bottom_when_jumping_near_end() {
    let screen = run_test_screen(TestCase {
        screen_width: 20,
        screen_height: 5,
        content: "\
line 1
line 2
line 3
line 4
target foo here
line 6",
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
    });
    // The matched line should be visible and the status line must be on
    // the last row (row 4), not immediately after the last content line.
    assert_eq!(
        screen.last_snapshot(),
        "\
line 3
line 4
{reverse}target{/reverse} foo here
line 6
:
"
    );
}

// When search jumps to a match near the end, downward scrolling should be
// blocked at the bottom, but upward scrolling should work normally.
#[test]
fn near_end_blocks_scroll_down_at_bottom() {
    let out = run_test_screen(TestCase {
        screen_width: 10,
        screen_height: 5,
        content: "\
line 1
line 2
line 3
line 4
line 5
line 6
line 7
line 8
line 9
line 10",
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
    })
    .out();
    assert_eq!(
        out,
        "\
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
/
-----
[EVENT]:char:9
line 7
line 8
line {reverse}9{/reverse}
line 10
/9
-----
[EVENT]:other
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
"
    );
}

// Verify final state after scrolling past a near-end match.
#[test]
fn near_end_scroll_up_then_down() {
    let screen = run_test_screen(TestCase {
        screen_width: 10,
        screen_height: 5,
        content: "\
line 1
line 2
line 3
line 4
line 5
line 6
line 7
line 8
line 9
line 10",
        events: vec![
            key('/'),
            key('9'),
            enter(),
            // Scroll up twice, then back down.
            key('k'),
            key('k'),
            key('j'),
        ],
    });
    assert_eq!(
        screen.last_snapshot(),
        "\
line 6
line 7
line 8
line {reverse}9{/reverse}
:
"
    );
}
