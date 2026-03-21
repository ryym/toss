use pretty_assertions::assert_eq;

use super::{TestCase, key, run_test_screen};
use crate::options::{Options, SectionOptions};

fn section_opts(pattern: &str) -> Option<SectionOptions> {
    Some(SectionOptions {
        pattern: regex::Regex::new(pattern).unwrap(),
        header_lines: 1,
    })
}

fn section_opts_n(pattern: &str, header_lines: usize) -> Option<SectionOptions> {
    Some(SectionOptions {
        pattern: regex::Regex::new(pattern).unwrap(),
        header_lines,
    })
}

/// No section header displayed when viewport is at the top
/// (section line is still visible in viewport, not scrolled past).
#[test]
fn no_sticky_when_section_visible() {
    let screen = run_test_screen(TestCase {
        screen_width: 20,
        screen_height: 5,
        content: "\
# Section A
line 1
line 2
line 3",
        options: Options {
            section: section_opts("^# "),
            ..Default::default()
        },
        events: vec![key('q')],
        ..Default::default()
    });
    assert_eq!(
        screen.last_snapshot(),
        "\
# Section A
line 1
line 2
line 3
:
"
    );
}

/// Section header appears as sticky when scrolled past.
#[test]
fn sticky_after_scroll_past_section() {
    let screen = run_test_screen(TestCase {
        screen_width: 20,
        screen_height: 5,
        content: "\
# Section A
line 1
line 2
line 3
line 4",
        options: Options {
            section: section_opts("^# "),
            ..Default::default()
        },
        // Scroll down 1 so "# Section A" is above viewport
        events: vec![key('j'), key('q')],
        ..Default::default()
    });
    // Header takes 1 row, viewport has 3 rows, status line 1 row = 5
    assert_eq!(
        screen.last_snapshot(),
        "\
# Section A
line 1
line 2
line 3
:
"
    );
}

/// Section header updates when scrolling into a new section.
#[test]
fn sticky_updates_on_new_section() {
    let screen = run_test_screen(TestCase {
        screen_width: 20,
        screen_height: 5,
        content: "\
# Section A
line 1
# Section B
line 2
line 3
line 4",
        options: Options {
            section: section_opts("^# "),
            ..Default::default()
        },
        // Scroll down 3 so both sections are above viewport
        events: vec![key('j'), key('j'), key('j'), key('q')],
        ..Default::default()
    });
    assert_eq!(
        screen.last_snapshot(),
        "\
# Section B
line 2
line 3
line 4
:
"
    );
}

/// Section header disappears when scrolling back up past the section start.
#[test]
fn sticky_disappears_on_scroll_up() {
    let screen = run_test_screen(TestCase {
        screen_width: 20,
        screen_height: 5,
        content: "\
# Section A
line 1
line 2
line 3
line 4",
        options: Options {
            section: section_opts("^# "),
            ..Default::default()
        },
        // Scroll down 1, then back up 1
        events: vec![key('j'), key('k'), key('q')],
        ..Default::default()
    });
    assert_eq!(
        screen.last_snapshot(),
        "\
# Section A
line 1
line 2
line 3
:
"
    );
}

/// Section header works with fixed header combined.
#[test]
fn section_with_fixed_header() {
    let screen = run_test_screen(TestCase {
        screen_width: 20,
        screen_height: 6,
        content: "\
FIXED
# Section A
line 1
line 2
# Section B
line 3
line 4",
        options: Options {
            header: 1,
            section: section_opts("^# "),
        },
        // Scroll down 3: viewport_top=4 (# Section B).
        // Section A (line 1) is sticky: 1+1<=4.
        // Section B (line 4) is visible but not yet above viewport.
        events: vec![key('j'), key('j'), key('j'), key('q')],
        ..Default::default()
    });
    assert_eq!(
        screen.last_snapshot(),
        "\
FIXED
# Section A
# Section B
line 3
line 4
:
"
    );
}

/// Multi-line section header (--section-header 2).
#[test]
fn multi_line_section_header() {
    let screen = run_test_screen(TestCase {
        screen_width: 20,
        screen_height: 7,
        content: "\
# Section A
description A
line 1
line 2
line 3
line 4",
        options: Options {
            section: section_opts_n("^# ", 2),
            ..Default::default()
        },
        // Scroll down 2: section header block [0,1] is fully above viewport
        // Sticky header = 2 rows, viewport = 7-1-2 = 4 rows
        events: vec![key('j'), key('j'), key('q')],
        ..Default::default()
    });
    assert_eq!(
        screen.last_snapshot(),
        "\
# Section A
description A
line 1
line 2
line 3
line 4
:
"
    );
}

/// Jump to end with section headers. Section A is sticky because
/// the viewport top is at the section B line itself (still visible).
#[test]
fn jump_end_with_section() {
    let screen = run_test_screen(TestCase {
        screen_width: 20,
        screen_height: 5,
        content: "\
# Section A
line 1
line 2
# Section B
line 3
line 4
line 5",
        options: Options {
            section: section_opts("^# "),
            ..Default::default()
        },
        events: vec![key('G'), key('q')],
        ..Default::default()
    });
    assert_eq!(
        screen.last_snapshot(),
        "\
# Section A
# Section B
line 3
line 4
:
"
    );
}

/// Scroll down enough so that section B is fully above viewport.
#[test]
fn section_b_becomes_sticky() {
    let screen = run_test_screen(TestCase {
        screen_width: 20,
        screen_height: 5,
        content: "\
# Section A
line 1
# Section B
line 3
line 4
line 5",
        options: Options {
            section: section_opts("^# "),
            ..Default::default()
        },
        // Scroll 3 down: viewport_top=3, section B (line 2): 2+1=3<=3, sticky
        events: vec![key('j'), key('j'), key('j'), key('q')],
        ..Default::default()
    });
    assert_eq!(
        screen.last_snapshot(),
        "\
# Section B
line 3
line 4
line 5
:
"
    );
}

/// No section above viewport means no sticky header.
#[test]
fn no_section_above_viewport() {
    let screen = run_test_screen(TestCase {
        screen_width: 20,
        screen_height: 5,
        content: "\
line 1
line 2
line 3
# Section A
line 4",
        options: Options {
            section: section_opts("^# "),
            ..Default::default()
        },
        // Scroll down 1: only regular lines above, no section
        events: vec![key('j'), key('q')],
        ..Default::default()
    });
    assert_eq!(
        screen.last_snapshot(),
        "\
line 2
line 3
# Section A
line 4
:
"
    );
}

/// Section header with fixed header where section line is within the fixed header range.
/// The section header should not duplicate the fixed header lines.
#[test]
fn section_overlaps_fixed_header() {
    let screen = run_test_screen(TestCase {
        screen_width: 20,
        screen_height: 6,
        content: "\
# Section A
line 1
line 2
line 3
line 4
line 5",
        options: Options {
            header: 1,
            section: section_opts("^# "),
        },
        // Scroll down 1
        events: vec![key('j'), key('q')],
        ..Default::default()
    });
    // Section A is at line 0 which is also the fixed header.
    // It should not appear twice.
    assert_eq!(
        screen.last_snapshot(),
        "\
# Section A
line 2
line 3
line 4
line 5
:
"
    );
}
