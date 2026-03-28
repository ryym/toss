use pretty_assertions::assert_eq;

use super::{TestCase, key, run_test_screen};
use crate::options::Options;

#[test]
fn header_pinned_at_top() {
    let content = "\
HEADER 1
HEADER 2
line 3
line 4
line 5
line 6
";
    let screen = run_test_screen(TestCase {
        screen_width: 10,
        screen_height: 5,
        content,
        options: Options {
            header: 2,
            ..Default::default()
        },
        events: vec![key('q')],
        ..Default::default()
    });
    let want = "\
HEADER 1
HEADER 2
line 3
line 4
:
";
    assert_eq!(screen.last_snapshot(), want);
}

#[test]
fn header_stays_after_scroll_down() {
    let content = "\
HEADER 1
HEADER 2
line 3
line 4
line 5
line 6
";
    let screen = run_test_screen(TestCase {
        screen_width: 10,
        screen_height: 5,
        content,
        options: Options {
            header: 2,
            ..Default::default()
        },
        events: vec![key('j'), key('j'), key('q')],
        ..Default::default()
    });
    let want = "\
HEADER 1
HEADER 2
line 5
line 6
:
";
    assert_eq!(screen.last_snapshot(), want);
}

#[test]
fn header_stays_after_scroll_up() {
    let content = "\
HEADER 1
HEADER 2
line 3
line 4
line 5
line 6
";
    let screen = run_test_screen(TestCase {
        screen_width: 10,
        screen_height: 5,
        content,
        options: Options {
            header: 2,
            ..Default::default()
        },
        events: vec![key('j'), key('j'), key('k'), key('q')],
        ..Default::default()
    });
    let want = "\
HEADER 1
HEADER 2
line 4
line 5
:
";
    assert_eq!(screen.last_snapshot(), want);
}

/// Scrolling up should not go above the header lines.
#[test]
fn cannot_scroll_above_header() {
    let content = "\
HEADER 1
HEADER 2
line 3
line 4
line 5
";
    let screen = run_test_screen(TestCase {
        screen_width: 10,
        screen_height: 5,
        content,
        options: Options {
            header: 2,
            ..Default::default()
        },
        events: vec![key('k'), key('k'), key('q')],
        ..Default::default()
    });
    let want = "\
HEADER 1
HEADER 2
line 3
line 4
:
";
    assert_eq!(screen.last_snapshot(), want);
}

/// 'g' should jump to the first non-header line.
#[test]
fn jump_to_top_respects_header() {
    let content = "\
HEADER
line 2
line 3
line 4
line 5
line 6
";
    let screen = run_test_screen(TestCase {
        screen_width: 10,
        screen_height: 5,
        content,
        options: Options {
            header: 1,
            ..Default::default()
        },
        events: vec![key('G'), key('g'), key('q')],
        ..Default::default()
    });
    let want = "\
HEADER
line 2
line 3
line 4
:
";
    assert_eq!(screen.last_snapshot(), want);
}

/// 'G' should jump to the end while keeping the header.
#[test]
fn jump_to_end_with_header() {
    let content = "\
HEADER
line 2
line 3
line 4
line 5
line 6
";
    let screen = run_test_screen(TestCase {
        screen_width: 10,
        screen_height: 5,
        content,
        options: Options {
            header: 1,
            ..Default::default()
        },
        events: vec![key('G'), key('q')],
        ..Default::default()
    });
    let want = "\
HEADER
line 4
line 5
line 6
:
";
    assert_eq!(screen.last_snapshot(), want);
}

/// header=0 should behave identically to no header.
#[test]
fn zero_header_is_noop() {
    let content = "\
line 1
line 2
line 3
line 4
line 5
";
    let screen = run_test_screen(TestCase {
        screen_width: 10,
        screen_height: 4,
        content,
        options: Options {
            header: 0,
            ..Default::default()
        },
        events: vec![key('q')],
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
