use pretty_assertions::assert_eq;

use super::{TestCase, enter, key, run_test_screen};
use crate::options::{HeadingOptions, Options};

fn heading_opts(pattern: &str) -> Option<HeadingOptions> {
    Some(HeadingOptions {
        pattern: regex::Regex::new(pattern).unwrap(),
        num_lines: 1,
    })
}

/// Section line is visible in the viewport at the top, so no sticky heading.
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
            heading: heading_opts("^# "),
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
{rev}lines 1-4/5 80%{/rev}
-----
[EVENT]:char:j
# Section A
line 2
line 3
line 4
{rev}lines 2-5/5 100%{/rev}
-----
[EVENT]:char:k
# Section A
line 1
line 2
line 3
{rev}lines 1-4/5 80%{/rev}
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
            heading: heading_opts("^# "),
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
{rev}lines 1-4/6 66%{/rev}
-----
[EVENT]:char:j
# Section A
# Section B
line 2
line 3
{rev}lines 2-5/6 83%{/rev}
-----
[EVENT]:char:j
# Section B
line 2
line 3
line 4
{rev}lines 3-6/6 100%{/rev}
-----
[EVENT]:char:q
";
    assert_eq!(screen.out(), want);
}

/// Fixed header stays, and heading appears below it when scrolled.
#[test]
fn heading_with_fixed_header() {
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
            heading: heading_opts("^# "),
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
{rev}lines 1-5/7 71%{/rev}
-----
[EVENT]:char:j
FIXED
# Section A
line 2
# Section B
line 3
{rev}lines 2-6/7 85%{/rev}
-----
[EVENT]:char:j
FIXED
# Section A
# Section B
line 3
line 4
{rev}lines 3-7/7 100%{/rev}
-----
[EVENT]:char:q
";
    assert_eq!(screen.out(), want);
}

/// Jump to end with headings. Section B is at the top of
/// the viewport, so section A's sticky heading is pushed off.
#[test]
fn jump_end_with_heading() {
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
            heading: heading_opts("^# "),
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
{rev}lines 1-4/7 57%{/rev}
-----
[EVENT]:char:G
# Section B
line 3
line 4
line 5
{rev}lines 4-7/7 100%{/rev}
-----
[EVENT]:char:q
";
    assert_eq!(screen.out(), want);
}

/// No section above viewport means no sticky heading.
#[test]
fn no_heading_above_viewport() {
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
            heading: heading_opts("^# "),
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
{rev}lines 1-4/5 80%{/rev}
-----
[EVENT]:char:j
line 2
line 3
# Section A
line 4
{rev}lines 2-5/5 100%{/rev}
-----
[EVENT]:char:q
";
    assert_eq!(screen.out(), want);
}

/// Heading with fixed header where section line is within the
/// fixed header range. The heading should not duplicate.
#[test]
fn heading_overlaps_fixed_header() {
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
            heading: heading_opts("^# "),
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
{rev}lines 1-5/6 83%{/rev}
-----
[EVENT]:char:j
# Section A
line 2
line 3
line 4
line 5
{rev}lines 2-6/6 100%{/rev}
-----
[EVENT]:char:q
";
    assert_eq!(screen.out(), want);
}

/// A heading right below a wrapped header still becomes sticky when scrolled past.
#[test]
fn heading_just_below_wrapped_header_line() {
    let content = "\
HEADERLINE!
# A
b1
b2
b3
b4
b5
b6
b7
b8
";
    let screen = run_test_screen(TestCase {
        screen_width: 8,
        screen_height: 7,
        content,
        options: Options {
            header: 1,
            heading: heading_opts("^# "),
            ..Default::default()
        },
        events: vec![key('G'), key('q')],
        ..Default::default()
    });
    let want = "\
HEADERLI>
NE!
# A
b1
b2
b3
{rev}5/10 50%{/rev}
-----
[EVENT]:char:G
HEADERLI>
NE!
# A
b6
b7
b8
{rev}/10 100%{/rev}
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
            heading: heading_opts("^# "),
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
{rev}lines 1-4/6 66%{/rev}
-----
[EVENT]:char:j
# Section A
# Section B
line 3
line 4
{rev}lines 2-5/6 83%{/rev}
-----
[EVENT]:char:j
# Section B
line 3
line 4
line 5
{rev}lines 3-6/6 100%{/rev}
-----
[EVENT]:char:j
[EVENT]:char:k
# Section A
# Section B
line 3
line 4
{rev}lines 2-5/6 83%{/rev}
-----
[EVENT]:char:k
# Section A
line 1
# Section B
line 3
{rev}lines 1-4/6 66%{/rev}
-----
[EVENT]:char:q
";
    assert_eq!(screen.out(), want);
}

/// Jumping to the end with a global header configured should pin the heading of the
/// section the visible content belongs to. The viewport top row after the jump is `a2`,
/// which the 2-row header covers, and `# B` sits in the covered rows too.
#[test]
fn jump_to_end_resolves_heading_below_header() {
    let content = "\
H1
H2
# A
a1
a2
# B
b1
b2
b3
b4
";
    let screen = run_test_screen(TestCase {
        screen_width: 20,
        screen_height: 7,
        content,
        options: Options {
            header: 2,
            heading: heading_opts("^# "),
            ..Default::default()
        },
        events: vec![key('G'), key('q')],
        ..Default::default()
    });
    let want = "\
H1
H2
# A
a1
a2
# B
{rev}lines 1-6/10 60%{/rev}
-----
[EVENT]:char:G
H1
H2
# B
b2
b3
b4
{rev}lines 5-10/10 100%{/rev}
-----
[EVENT]:char:q
";
    assert_eq!(screen.out(), want);
}

/// Jumping to a match below the page (`n`) must pin the heading of the section the visible
/// content belongs to, like jumping to the end does. The viewport top row after the jump is
/// `az` and `# B` is the next row, both covered by the 2-row header.
#[test]
fn jump_to_match_below_resolves_heading_below_header() {
    let content = "\
H1
H2
# A
az
# B
b1
b2
b3
zz
b4
";
    let screen = run_test_screen(TestCase {
        screen_width: 20,
        screen_height: 7,
        content,
        options: Options {
            header: 2,
            heading: heading_opts("^# "),
            ..Default::default()
        },
        events: vec![key('/'), key('z'), enter(), key('n'), key('q')],
        ..Default::default()
    });
    let want = "\
H1
H2
# A
az
# B
b1
{rev}lines 1-6/10 60%{/rev}
-----
[EVENT]:char:/
H1
H2
# A
az
# B
b1
/█
-----
[EVENT]:char:z
H1
H2
# A
a{rev}{b}z{/rev}{/b}
# B
b1
/z█
-----
[EVENT]:enter
H1
H2
# A
a{rev}{b}z{/rev}{/b}
# B
b1
{rev}lines 1-6/10 60%{/rev}
-----
[EVENT]:char:n
H1
H2
# B
b2
b3
{rev}{b}z{/rev}{/b}{rev}{line}{b}z{/rev}{/line}{/b}
{rev}lines 4-9/10 90%{/rev}
-----
[EVENT]:char:q
";
    assert_eq!(screen.out(), want);
}
