//! Resize behavior of the sticky heading.
//! See `dev/issues/draft/heading-state-not-recomputed-on-resize.md`.

use pretty_assertions::assert_eq;

use super::{TestCase, key, resize, run_test_screen};
use crate::options::{HeadingOptions, Options};

fn heading_opts_n(pattern: &str, num_lines: usize) -> Option<HeadingOptions> {
    Some(HeadingOptions {
        pattern: regex::Regex::new(pattern).unwrap(),
        num_lines,
    })
}

const CONTENT: &str = "\
# A
a1
a2
a3
body a1
body a2
# B
b1
b2
b3
body b1
body b2
tail
";

/// Shrinking the screen while the heading is pushed up should rebuild the heading
/// for the new size. Here `# B` has pushed heading A up by 3 of its 4 rows, and the
/// new size leaves room for only 1 heading row, so heading A is shown from its start
/// again with `# B` as the only content row below it.
#[test]
// BUG: the stale push-up offset (3) is applied to the rebuilt 1-row heading,
// so `Heading::rows` panics with
// "range start index 3 out of range for slice of length 1".
#[should_panic]
fn shrink_after_push_up_rebuilds_heading() {
    let screen = run_test_screen(TestCase {
        screen_width: 20,
        screen_height: 8,
        content: CONTENT,
        options: Options {
            heading: heading_opts_n("^# ", 4),
            ..Default::default()
        },
        events: vec![
            key('j'),
            key('j'),
            key('j'),
            key('j'),
            key('j'),
            resize(20, 3),
            key('q'),
        ],
        ..Default::default()
    });
    let want = "\
# A
a1
a2
a3
body a1
body a2
# B
{rev}lines 1-7/13 53%{/rev}
-----
[EVENT]:char:j
# A
a1
a2
a3
body a2
# B
b1
{rev}lines 2-8/13 61%{/rev}
-----
[EVENT]:char:j
# A
a1
a2
a3
# B
b1
b2
{rev}lines 3-9/13 69%{/rev}
-----
[EVENT]:char:j
a1
a2
a3
# B
b1
b2
b3
{rev}lines 4-10/13 76%{/rev}
-----
[EVENT]:char:j
a2
a3
# B
b1
b2
b3
body b1
{rev}lines 5-11/13 84%{/rev}
-----
[EVENT]:char:j
a3
# B
b1
b2
b3
body b1
body b2
{rev}lines 6-12/13 92%{/rev}
-----
[EVENT]:resize:20x3
# A
# B
{rev}lines 6-7/13 53%{/rev}
-----
[EVENT]:char:q
";
    assert_eq!(screen.out(), want);
}

/// Shrinking the screen should re-derive how far the heading is pushed up.
///
/// The overlay is 3 rows tall both before and after the resize, and `# B` is the first
/// content row in both cases. What differs is *why* it is 3 rows, and that decides which
/// 3 rows of heading A are painted into it:
///
/// - Before: the heading is 4 rows (`--heading-lines 4`), which would cover `# B` at
///   viewport row 3, so it is pushed up by 1. A push-up drops rows from the *start*, so
///   `# A` is the row that goes: `a1` / `a2` / `a3`.
/// - After: `max_heading_height` caps the heading at 3 rows, because at least one content
///   row must stay visible. The overlay no longer reaches `# B`, so no push-up is needed.
///   The cap drops lines from the *end*, so `a3` is the row that goes: `# A` / `a1` / `a2`.
///
/// The result is what scrolling to the same top line on a 20x5 screen produces from the
/// start: the page is a function of the top line and the size, not of the size it came from.
#[test]
// BUG: the push-up offset (1) survives the resize, so the heading is rendered from
// its second row (`a1`) and one more content row (`body a2`) leaks in below it:
//   a1 / a2 / body a2 / # B
#[should_panic]
fn shrink_recomputes_push_up_offset() {
    let screen = run_test_screen(TestCase {
        screen_width: 20,
        screen_height: 8,
        content: CONTENT,
        options: Options {
            heading: heading_opts_n("^# ", 4),
            ..Default::default()
        },
        events: vec![key('j'), key('j'), key('j'), resize(20, 5), key('q')],
        ..Default::default()
    });
    let want = "\
# A
a1
a2
a3
body a1
body a2
# B
{rev}lines 1-7/13 53%{/rev}
-----
[EVENT]:char:j
# A
a1
a2
a3
body a2
# B
b1
{rev}lines 2-8/13 61%{/rev}
-----
[EVENT]:char:j
# A
a1
a2
a3
# B
b1
b2
{rev}lines 3-9/13 69%{/rev}
-----
[EVENT]:char:j
a1
a2
a3
# B
b1
b2
b3
{rev}lines 4-10/13 76%{/rev}
-----
[EVENT]:resize:20x5
# A
a1
a2
# B
{rev}lines 4-7/13 53%{/rev}
-----
[EVENT]:char:q
";
    assert_eq!(screen.out(), want);
}

/// A heading truncated to fit a small screen should grow back to its configured
/// `--heading-lines` once the screen has room again. The 20x4 screen fits only 2 of
/// the 3 heading lines; the 20x10 screen fits all of them.
#[test]
// BUG: the heading keeps the line range captured for the small screen, so it stays
// 2 rows (`# A` / `a1`) and `a2` is never shown as part of the heading.
#[should_panic]
fn grow_restores_full_heading_height() {
    let screen = run_test_screen(TestCase {
        screen_width: 20,
        screen_height: 4,
        content: CONTENT,
        options: Options {
            heading: heading_opts_n("^# ", 3),
            ..Default::default()
        },
        events: vec![key('j'), key('j'), resize(20, 10), key('q')],
        ..Default::default()
    });
    let want = "\
# A
a1
a2
{rev}lines 1-3/13 23%{/rev}
-----
[EVENT]:char:j
# A
a1
a3
{rev}lines 2-4/13 30%{/rev}
-----
[EVENT]:char:j
# A
a1
body a1
{rev}lines 3-5/13 38%{/rev}
-----
[EVENT]:resize:20x10
# A
a1
a2
body a2
# B
b1
b2
b3
body b1
{rev}lines 3-11/13 84%{/rev}
-----
[EVENT]:char:q
";
    assert_eq!(screen.out(), want);
}

/// Shrinking the width so the header line wraps into two rows must not disqualify the
/// heading right below it from becoming sticky.
#[test]
fn resize_wraps_header_line_heading_stays_below_it() {
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
        screen_width: 20,
        screen_height: 5,
        content,
        options: Options {
            header: 1,
            heading: heading_opts_n("^# ", 1),
            ..Default::default()
        },
        events: vec![resize(8, 5), key('G'), key('q')],
        ..Default::default()
    });
    let want = "\
HEADERLINE!
# A
b1
b2
{rev}lines 1-4/10 40%{/rev}
-----
[EVENT]:resize:8x5
HEADERLI>
NE!
# A
b1
{rev}3/10 30%{/rev}
-----
[EVENT]:char:G
HEADERLI>
NE!
# A
b8
{rev}/10 100%{/rev}
-----
[EVENT]:char:q
";
    assert_eq!(screen.out(), want);
}

/// Lines inside the global header never become a heading, even when they match the
/// pattern. Here the header covers the whole document, so no heading is shown at any
/// screen size.
#[test]
fn resize_when_document_fits_entirely_in_header() {
    let content = "\
# A
h2
h3
";
    let screen = run_test_screen(TestCase {
        screen_width: 20,
        screen_height: 5,
        content,
        options: Options {
            header: 3,
            heading: heading_opts_n("^# ", 1),
            ..Default::default()
        },
        events: vec![resize(20, 7), key('q')],
        ..Default::default()
    });
    let want = "\
# A
h2
h3
{rev}lines 1-3/3 100%{/rev}

-----
[EVENT]:resize:20x7
# A
h2
h3
{rev}lines 1-3/3 100%{/rev}



-----
[EVENT]:char:q
";
    assert_eq!(screen.out(), want);
}

const WRAP_CONTENT: &str = "\
# A
sub a
long body 1
# B
sub b
body b1
body b2
body b3
tail
";

/// Changing only the width should re-derive the push-up too. The heading itself is
/// unaffected — `# A` / `sub a` fit on one row at either width, so it stays 2 rows — but
/// `long body 1` wraps into 2 rows at width 8, which moves `# B` one row further down.
/// It then sits below the overlay instead of inside it, so the heading is no longer
/// pushed up and is shown from its start.
#[test]
// BUG: the push-up offset (1) survives the resize, so the heading keeps showing only its
// second row and the tail of the wrapped line leaks in below it:
//   sub a / y 1 / # B
#[should_panic]
fn width_change_recomputes_push_up_offset() {
    let screen = run_test_screen(TestCase {
        screen_width: 12,
        screen_height: 7,
        content: WRAP_CONTENT,
        options: Options {
            heading: heading_opts_n("^# ", 2),
            ..Default::default()
        },
        events: vec![key('j'), key('j'), resize(8, 7), key('q')],
        ..Default::default()
    });
    let want = "\
# A
sub a
long body 1
# B
sub b
body b1
{rev}es 1-6/9 66%{/rev}
-----
[EVENT]:char:j
# A
sub a
# B
sub b
body b1
body b2
{rev}es 2-7/9 77%{/rev}
-----
[EVENT]:char:j
sub a
# B
sub b
body b1
body b2
body b3
{rev}es 3-8/9 88%{/rev}
-----
[EVENT]:resize:8x7
# A
sub a
# B
sub b
body b1
body b2
{rev}-7/9 77%{/rev}
-----
[EVENT]:char:q
";
    assert_eq!(screen.out(), want);
}
