use pretty_assertions::assert_eq;

use super::{TestCase, key, run_test};

/// `)` moves section by section with each heading pinned at the top, and stops at the last
/// section. `(` then walks back through the same pages.
#[test]
fn heading_jump_moves_between_sections() {
    let content = "\
# A
a1
a2
a3
# B
b1
b2
b3
# C
c1
c2
c3
c4
c5
c6
";
    let result = run_test(TestCase {
        args: vec!["--heading", "^#"],
        screen_width: 20,
        screen_height: 5,
        content,
        events: vec![key(')'), key(')'), key(')'), key('('), key('('), key('q')],
        ..Default::default()
    });
    let want = "\
# A
a1
a2
a3
{rev}lines 1-4/15 26%{/rev}
-----
[EVENT]:char:)
# B
b1
b2
b3
{rev}lines 5-8/15 53%{/rev}
-----
[EVENT]:char:)
# C
c1
c2
c3
{rev}lines 9-12/15 80%{/rev}
-----
[EVENT]:char:)
[EVENT]:char:(
# B
b1
b2
b3
{rev}lines 5-8/15 53%{/rev}
-----
[EVENT]:char:(
# A
a1
a2
a3
{rev}lines 1-4/15 26%{/rev}
-----
[EVENT]:char:q
";
    assert_eq!(result.output(), want);
}

/// With `--heading-lines 2` the whole heading block is pinned after a jump. Scrolling one
/// line into the block and pressing `(` returns to the start of the same heading.
#[test]
fn heading_jump_pins_the_whole_multi_line_heading() {
    let content = "\
# A
a-sub
a1
a2
# B
b-sub
b1
b2
b3
b4
b5
";
    let result = run_test(TestCase {
        args: vec!["--heading", "^#", "--heading-lines", "2"],
        screen_width: 20,
        screen_height: 5,
        content,
        events: vec![key(')'), key('j'), key('('), key('q')],
        ..Default::default()
    });
    let want = "\
# A
a-sub
a1
a2
{rev}lines 1-4/11 36%{/rev}
-----
[EVENT]:char:)
# B
b-sub
b1
b2
{rev}lines 5-8/11 72%{/rev}
-----
[EVENT]:char:j
# B
b-sub
b2
b3
{rev}lines 6-9/11 81%{/rev}
-----
[EVENT]:char:(
# B
b-sub
b1
b2
{rev}lines 5-8/11 72%{/rev}
-----
[EVENT]:char:q
";
    assert_eq!(result.output(), want);
}

/// Without `--heading` there is no section to move between, so the page stays.
#[test]
fn heading_jump_does_nothing_without_the_heading_option() {
    let content = "\
# A
a1
a2
# B
b1
b2
";
    let result = run_test(TestCase {
        screen_width: 20,
        screen_height: 5,
        content,
        events: vec![key(')'), key('q')],
        ..Default::default()
    });
    let want = "\
# A
a1
a2
# B
{rev}lines 1-4/6 66%{/rev}
-----
[EVENT]:char:)
[EVENT]:char:q
";
    assert_eq!(result.output(), want);
}

/// In the middle of a section, `(` first returns to its start and only then moves on to the
/// previous section. Above the first heading there is nowhere to go.
#[test]
fn heading_jump_up_returns_to_the_section_start_first() {
    let content = "\
# A
a1
a2
a3
# B
b1
b2
b3
b4
b5
b6
";
    let result = run_test(TestCase {
        args: vec!["--heading", "^#"],
        screen_width: 20,
        screen_height: 5,
        content,
        events: vec![
            key(')'),
            key('j'),
            key('j'),
            key('('),
            key('('),
            key('('),
            key('q'),
        ],
        ..Default::default()
    });
    let want = "\
# A
a1
a2
a3
{rev}lines 1-4/11 36%{/rev}
-----
[EVENT]:char:)
# B
b1
b2
b3
{rev}lines 5-8/11 72%{/rev}
-----
[EVENT]:char:j
# B
b2
b3
b4
{rev}lines 6-9/11 81%{/rev}
-----
[EVENT]:char:j
# B
b3
b4
b5
{rev}lines 7-10/11 90%{/rev}
-----
[EVENT]:char:(
# B
b1
b2
b3
{rev}lines 5-8/11 72%{/rev}
-----
[EVENT]:char:(
# A
a1
a2
a3
{rev}lines 1-4/11 36%{/rev}
-----
[EVENT]:char:(
[EVENT]:char:q
";
    assert_eq!(result.output(), want);
}

/// A heading line that wraps is pinned with all of its rows. Scrolling one row into it and
/// pressing `(` returns to the start of the same heading, not the previous one.
#[test]
fn heading_jump_up_within_a_wrapped_heading_returns_to_its_start() {
    let content = "\
# A
a1
a2
# B-long-heading
b1
b2
b3
b4
";
    let result = run_test(TestCase {
        args: vec!["--heading", "^#"],
        screen_width: 10,
        screen_height: 6,
        content,
        events: vec![key(')'), key('j'), key('('), key('q')],
        ..Default::default()
    });
    let want = "\
# A
a1
a2
# B-long-h>
eading
{rev} 1-4/8 50%{/rev}
-----
[EVENT]:char:)
# B-long-h>
eading
b1
b2
b3
{rev} 4-7/8 87%{/rev}
-----
[EVENT]:char:j
# B-long-h>
eading
b2
b3
b4
{rev}4-8/8 100%{/rev}
-----
[EVENT]:char:(
# B-long-h>
eading
b1
b2
b3
{rev} 4-7/8 87%{/rev}
-----
[EVENT]:char:q
";
    assert_eq!(result.output(), want);
}

/// A heading on the last page cannot be brought to the top, so `)` lands with it lower in
/// the page and goes no further, though another heading follows on the same page.
#[test]
fn heading_jump_stops_at_the_last_page() {
    let content = "\
# A
a1
a2
a3
a4
a5
a6
a7
# B
b1
# C
c1
";
    let result = run_test(TestCase {
        args: vec!["--heading", "^#"],
        screen_width: 20,
        screen_height: 6,
        content,
        events: vec![key(')'), key(')'), key('q')],
        ..Default::default()
    });
    let want = "\
# A
a1
a2
a3
a4
{rev}lines 1-5/12 41%{/rev}
-----
[EVENT]:char:)
# A
# B
b1
# C
c1
{rev}lines 8-12/12 100%{/rev}
-----
[EVENT]:char:)
[EVENT]:char:q
";
    assert_eq!(result.output(), want);
}

/// A line within `--header` never counts as a heading, even when it matches the pattern.
#[test]
fn heading_jump_ignores_the_header() {
    let content = "\
title
# H
x1
x2
x3
x4
# B
b1
b2
b3
b4
b5
";
    let result = run_test(TestCase {
        args: vec!["--header", "2", "--heading", "^#"],
        screen_width: 20,
        screen_height: 7,
        content,
        events: vec![key('('), key(')'), key('('), key('q')],
        ..Default::default()
    });
    let want = "\
title
# H
x1
x2
x3
x4
{rev}lines 1-6/12 50%{/rev}
-----
[EVENT]:char:(
[EVENT]:char:)
title
# H
# B
b1
b2
b3
{rev}lines 5-10/12 83%{/rev}
-----
[EVENT]:char:(
[EVENT]:char:q
";
    assert_eq!(result.output(), want);
}

/// When `--header` leaves a single row for the content, no heading can be pinned. Jumps still
/// work and bring the heading line to that row.
#[test]
fn heading_jump_works_without_room_to_pin_a_heading() {
    let content = "\
h1
h2
h3
x1
x2
# A
a1
a2
a3
# B
b1
b2
";
    let result = run_test(TestCase {
        args: vec!["--header", "3", "--heading", "^#"],
        screen_width: 20,
        screen_height: 5,
        content,
        events: vec![key(')'), key(')'), key('j'), key('('), key('('), key('q')],
        ..Default::default()
    });
    let want = "\
h1
h2
h3
x1
{rev}lines 1-4/12 33%{/rev}
-----
[EVENT]:char:)
h1
h2
h3
# A
{rev}lines 3-6/12 50%{/rev}
-----
[EVENT]:char:)
h1
h2
h3
# B
{rev}lines 7-10/12 83%{/rev}
-----
[EVENT]:char:j
h1
h2
h3
b1
{rev}lines 8-11/12 91%{/rev}
-----
[EVENT]:char:(
h1
h2
h3
# B
{rev}lines 7-10/12 83%{/rev}
-----
[EVENT]:char:(
h1
h2
h3
# A
{rev}lines 3-6/12 50%{/rev}
-----
[EVENT]:char:q
";
    assert_eq!(result.output(), want);
}
