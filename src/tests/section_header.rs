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

/// Section line is visible in the viewport at the top, so no sticky header.
/// Scrolling down 1 makes it sticky. Scrolling back up removes the sticky.
#[test]
fn sticky_appears_and_disappears_on_scroll() {
    let out = run_test_screen(TestCase {
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
        events: vec![key('j'), key('k'), key('q')],
        ..Default::default()
    })
    .out();
    assert_eq!(
        out,
        "\
# Section A
line 1
line 2
line 3
:
-----
[EVENT]:char:j
# Section A
line 1
line 2
line 3
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
"
    );
}

/// Scrolling through two sections: section A becomes sticky first,
/// then section B replaces it.
#[test]
fn sticky_transitions_between_sections() {
    let out = run_test_screen(TestCase {
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
        events: vec![key('j'), key('j'), key('j'), key('q')],
        ..Default::default()
    })
    .out();
    assert_eq!(
        out,
        "\
# Section A
line 1
# Section B
line 2
:
-----
[EVENT]:char:j
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
"
    );
}

/// Fixed header stays, and section header appears below it when scrolled.
#[test]
fn section_with_fixed_header() {
    let out = run_test_screen(TestCase {
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
        events: vec![key('j'), key('j'), key('j'), key('q')],
        ..Default::default()
    })
    .out();
    assert_eq!(
        out,
        "\
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
"
    );
}

/// Multi-line section header (--section-header 2).
#[test]
fn multi_line_section_header() {
    let out = run_test_screen(TestCase {
        screen_width: 20,
        screen_height: 6,
        content: "\
# Section A
description A
line 1
line 2
line 3
line 4
line 5",
        options: Options {
            section: section_opts_n("^# ", 2),
            ..Default::default()
        },
        // height=6, status=1, no section initially → viewport=5, content=7 lines
        // j: viewport_top=1, block [0,1] partially visible, no sticky
        // jj: viewport_top=2, block [0,1] fully above → sticky, header=2, viewport=3
        events: vec![key('j'), key('j'), key('q')],
        ..Default::default()
    })
    .out();
    assert_eq!(
        out,
        "\
# Section A
description A
line 1
line 2
line 3
:
-----
[EVENT]:char:j
# Section A
description A
line 2
line 3
line 4
:
-----
[EVENT]:char:j
# Section A
description A
line 3
line 4
line 5
:
-----
[EVENT]:char:q
"
    );
}

/// Multi-line section header (--section-header 2).
/// The sections switches gradually.
#[test]
fn multi_line_section_header_switching() {
    let out = run_test_screen(TestCase {
        screen_width: 20,
        screen_height: 6,
        content: "\
# Section A
description A
line 1
line 2
line 3
line 4
# Section B
description B
line 5
line 6
line 7
line 8
line 10",
        options: Options {
            section: section_opts_n("^# ", 2),
            ..Default::default()
        },
        events: vec![
            key('j'),
            key('j'),
            key('j'),
            key('j'),
            key('j'),
            key('j'),
            key('q'),
        ],
        ..Default::default()
    })
    .out();
    assert_eq!(
        out,
        "\
# Section A
description A
line 1
line 2
line 3
:
-----
[EVENT]:char:j
# Section A
description A
line 2
line 3
line 4
:
-----
[EVENT]:char:j
# Section A
description A
line 3
line 4
# Section B
:
-----
[EVENT]:char:j
# Section A
description A
line 4
# Section B
description B
:
-----
[EVENT]:char:j
# Section A
description A
# Section B
description B
line 5
:
-----
[EVENT]:char:j
description A
# Section B
description B
line 5
line 6
:
-----
[EVENT]:char:j
# Section B
description B
line 5
line 6
line 7
:
-----
[EVENT]:char:q
"
    );
}

/// Jump to end with section headers. Section A is sticky because
/// the section B line is still visible in the viewport.
#[test]
fn jump_end_with_section() {
    let out = run_test_screen(TestCase {
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
    })
    .out();
    assert_eq!(
        out,
        "\
# Section A
line 1
line 2
# Section B
:
-----
[EVENT]:char:G
# Section A
# Section B
line 3
line 4
:
-----
[EVENT]:char:q
"
    );
}

/// No section above viewport means no sticky header.
#[test]
fn no_section_above_viewport() {
    let out = run_test_screen(TestCase {
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
        events: vec![key('j'), key('q')],
        ..Default::default()
    })
    .out();
    assert_eq!(
        out,
        "\
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
"
    );
}

/// Section header with fixed header where section line is within the
/// fixed header range. The section header should not duplicate.
#[test]
fn section_overlaps_fixed_header() {
    let out = run_test_screen(TestCase {
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
        events: vec![key('j'), key('q')],
        ..Default::default()
    })
    .out();
    // Section A at line 0 overlaps with fixed header (also line 0).
    // It should not appear twice.
    assert_eq!(
        out,
        "\
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
"
    );
}

/// Scroll down enough so that section B is fully above viewport,
/// then scroll back up to see section A become sticky again.
#[test]
fn scroll_down_and_up_across_sections() {
    let out = run_test_screen(TestCase {
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
        events: vec![key('j'), key('j'), key('j'), key('k'), key('k'), key('q')],
        ..Default::default()
    })
    .out();
    assert_eq!(
        out,
        "\
# Section A
line 1
# Section B
line 3
:
-----
[EVENT]:char:j
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
"
    );
}
