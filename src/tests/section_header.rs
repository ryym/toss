use pretty_assertions::assert_eq;

use super::{TestCase, key, run_test_screen};
use crate::options::{Options, SectionOptions};

fn section_opts(pattern: &str) -> Option<SectionOptions> {
    Some(SectionOptions {
        pattern: regex::Regex::new(pattern).unwrap(),
        header_lines: 1,
    })
}

/// Section line is visible in the viewport at the top, so no sticky header.
/// Scrolling down 1 makes it sticky. Scrolling back up removes the sticky.
#[test]
fn sticky_appears_and_disappears_on_scroll() {
    let content = "\
# Section A
line 1
line 2
line 3
line 4
";
    let screen = run_test_screen(TestCase {
        screen_width: 20,
        screen_height: 5,
        content,
        options: Options {
            section: section_opts("^# "),
            ..Default::default()
        },
        events: vec![key('j'), key('k'), key('q')],
        ..Default::default()
    });
    let want = "\
# Section A
line 1
line 2
line 3
:
-----
[EVENT]:char:j
# Section A
line 2
line 3
line 4
:
-----
[EVENT]:char:k
# Section A
line 1
line 2
line 3
:
-----
[EVENT]:char:q
";
    assert_eq!(screen.out(), want);
}

/// Scrolling through two sections: section A becomes sticky first,
/// then section B replaces it.
#[test]
fn sticky_transitions_between_sections() {
    let content = "\
# Section A
line 1
# Section B
line 2
line 3
line 4
";
    let screen = run_test_screen(TestCase {
        screen_width: 20,
        screen_height: 5,
        content,
        options: Options {
            section: section_opts("^# "),
            ..Default::default()
        },
        events: vec![key('j'), key('j'), key('q')],
        ..Default::default()
    });
    let want = "\
# Section A
line 1
# Section B
line 2
:
-----
[EVENT]:char:j
# Section A
# Section B
line 2
line 3
:
-----
[EVENT]:char:j
# Section B
line 2
line 3
line 4
:
-----
[EVENT]:char:q
";
    assert_eq!(screen.out(), want);
}

/// Fixed header stays, and section header appears below it when scrolled.
#[test]
fn section_with_fixed_header() {
    let content = "\
FIXED
# Section A
line 1
line 2
# Section B
line 3
line 4
";
    let screen = run_test_screen(TestCase {
        screen_width: 20,
        screen_height: 6,
        content,
        options: Options {
            header: 1,
            section: section_opts("^# "),
            ..Default::default()
        },
        events: vec![key('j'), key('j'), key('q')],
        ..Default::default()
    });
    let want = "\
FIXED
# Section A
line 1
line 2
# Section B
:
-----
[EVENT]:char:j
FIXED
# Section A
line 2
# Section B
line 3
:
-----
[EVENT]:char:j
FIXED
# Section A
# Section B
line 3
line 4
:
-----
[EVENT]:char:q
";
    assert_eq!(screen.out(), want);
}

/// Jump to end with section headers. Section B is at the top of
/// the viewport, so section A's sticky header is pushed off.
#[test]
fn jump_end_with_section() {
    let content = "\
# Section A
line 1
line 2
# Section B
line 3
line 4
line 5
";
    let screen = run_test_screen(TestCase {
        screen_width: 20,
        screen_height: 5,
        content,
        options: Options {
            section: section_opts("^# "),
            ..Default::default()
        },
        events: vec![key('G'), key('q')],
        ..Default::default()
    });
    let want = "\
# Section A
line 1
line 2
# Section B
:
-----
[EVENT]:char:G
# Section B
line 3
line 4
line 5
:
-----
[EVENT]:char:q
";
    assert_eq!(screen.out(), want);
}

/// No section above viewport means no sticky header.
#[test]
fn no_section_above_viewport() {
    let content = "\
line 1
line 2
line 3
# Section A
line 4
";
    let screen = run_test_screen(TestCase {
        screen_width: 20,
        screen_height: 5,
        content,
        options: Options {
            section: section_opts("^# "),
            ..Default::default()
        },
        events: vec![key('j'), key('q')],
        ..Default::default()
    });
    let want = "\
line 1
line 2
line 3
# Section A
:
-----
[EVENT]:char:j
line 2
line 3
# Section A
line 4
:
-----
[EVENT]:char:q
";
    assert_eq!(screen.out(), want);
}

/// Section header with fixed header where section line is within the
/// fixed header range. The section header should not duplicate.
#[test]
fn section_overlaps_fixed_header() {
    let content = "\
# Section A
line 1
line 2
line 3
line 4
line 5
";
    let screen = run_test_screen(TestCase {
        screen_width: 20,
        screen_height: 6,
        content,
        options: Options {
            header: 1,
            section: section_opts("^# "),
            ..Default::default()
        },
        events: vec![key('j'), key('q')],
        ..Default::default()
    });
    // Section A at line 0 overlaps with fixed header (also line 0).
    // It should not appear twice.
    let want = "\
# Section A
line 1
line 2
line 3
line 4
:
-----
[EVENT]:char:j
# Section A
line 2
line 3
line 4
line 5
:
-----
[EVENT]:char:q
";
    assert_eq!(screen.out(), want);
}

/// Scroll down enough so that section B is fully above viewport,
/// then scroll back up to see section A become sticky again.
#[test]
fn scroll_down_and_up_across_sections() {
    let content = "\
# Section A
line 1
# Section B
line 3
line 4
line 5
";
    let screen = run_test_screen(TestCase {
        screen_width: 20,
        screen_height: 5,
        content,
        options: Options {
            section: section_opts("^# "),
            ..Default::default()
        },
        events: vec![key('j'), key('j'), key('j'), key('k'), key('k'), key('q')],
        ..Default::default()
    });
    let want = "\
# Section A
line 1
# Section B
line 3
:
-----
[EVENT]:char:j
# Section A
# Section B
line 3
line 4
:
-----
[EVENT]:char:j
# Section B
line 3
line 4
line 5
:
-----
[EVENT]:char:j
[EVENT]:char:k
# Section A
# Section B
line 3
line 4
:
-----
[EVENT]:char:k
# Section A
line 1
# Section B
line 3
:
-----
[EVENT]:char:q
";
    assert_eq!(screen.out(), want);
}
