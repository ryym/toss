use pretty_assertions::assert_eq;

use super::{TestCase, key, run_test_screen};
use crate::options::Options;

#[test]
fn header_pinned_at_top() {
    let screen = run_test_screen(TestCase {
        screen_width: 10,
        screen_height: 5,
        content: "\
HEADER 1
HEADER 2
line 3
line 4
line 5
line 6",
        options: Options { header: 2 },
        events: vec![key('q')],
        ..Default::default()
    });
    assert_eq!(
        screen.last_snapshot(),
        "\
HEADER 1
HEADER 2
line 3
line 4
:
"
    );
}

#[test]
fn header_stays_after_scroll_down() {
    let screen = run_test_screen(TestCase {
        screen_width: 10,
        screen_height: 5,
        content: "\
HEADER 1
HEADER 2
line 3
line 4
line 5
line 6",
        options: Options { header: 2 },
        events: vec![key('j'), key('j'), key('q')],
        ..Default::default()
    });
    assert_eq!(
        screen.last_snapshot(),
        "\
HEADER 1
HEADER 2
line 5
line 6
:
"
    );
}

#[test]
fn header_stays_after_scroll_up() {
    let screen = run_test_screen(TestCase {
        screen_width: 10,
        screen_height: 5,
        content: "\
HEADER 1
HEADER 2
line 3
line 4
line 5
line 6",
        options: Options { header: 2 },
        events: vec![key('j'), key('j'), key('k'), key('q')],
        ..Default::default()
    });
    assert_eq!(
        screen.last_snapshot(),
        "\
HEADER 1
HEADER 2
line 4
line 5
:
"
    );
}

/// Scrolling up should not go above the header lines.
#[test]
fn cannot_scroll_above_header() {
    let screen = run_test_screen(TestCase {
        screen_width: 10,
        screen_height: 5,
        content: "\
HEADER 1
HEADER 2
line 3
line 4
line 5",
        options: Options { header: 2 },
        events: vec![key('k'), key('k'), key('q')],
        ..Default::default()
    });
    assert_eq!(
        screen.last_snapshot(),
        "\
HEADER 1
HEADER 2
line 3
line 4
:
"
    );
}

/// 'g' should jump to the first non-header line.
#[test]
fn jump_to_top_respects_header() {
    let screen = run_test_screen(TestCase {
        screen_width: 10,
        screen_height: 5,
        content: "\
HEADER
line 2
line 3
line 4
line 5
line 6",
        options: Options { header: 1 },
        events: vec![key('G'), key('g'), key('q')],
        ..Default::default()
    });
    assert_eq!(
        screen.last_snapshot(),
        "\
HEADER
line 2
line 3
line 4
:
"
    );
}

/// 'G' should jump to the end while keeping the header.
#[test]
fn jump_to_end_with_header() {
    let screen = run_test_screen(TestCase {
        screen_width: 10,
        screen_height: 5,
        content: "\
HEADER
line 2
line 3
line 4
line 5
line 6",
        options: Options { header: 1 },
        events: vec![key('G'), key('q')],
        ..Default::default()
    });
    assert_eq!(
        screen.last_snapshot(),
        "\
HEADER
line 4
line 5
line 6
:
"
    );
}

/// header=0 should behave identically to no header.
#[test]
fn zero_header_is_noop() {
    let screen = run_test_screen(TestCase {
        screen_width: 10,
        screen_height: 4,
        content: "\
line 1
line 2
line 3
line 4
line 5",
        options: Options { header: 0 },
        events: vec![key('q')],
        ..Default::default()
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
