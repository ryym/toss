use pretty_assertions::assert_eq;

use super::{TestCase, enter, key, run_test_screen};
use crate::options::{HeadingOptions, Options};

fn heading_opts(pattern: &str, num_lines: usize) -> Option<HeadingOptions> {
    Some(HeadingOptions {
        pattern: regex::Regex::new(pattern).unwrap(),
        num_lines,
    })
}

/// When searching with a global header,
/// the matched line is visible below the header, not hidden behind it.
#[test]
fn search_with_global_header() {
    let content = "\
# Title
line 1
line 2
line 3
line 4
line 5
";
    let screen = run_test_screen(TestCase {
        screen_width: 20,
        screen_height: 4,
        content,
        options: Options {
            header: 1,
            ..Default::default()
        },
        events: vec![key('/'), key('3'), enter(), key('q')],
        ..Default::default()
    });
    let want = "\
# Title
line 1
line 2
{rev}lines 1-3/6 50%{/rev}
-----
[EVENT]:char:/
# Title
line 1
line 2
/█
-----
[EVENT]:char:3
# Title
line {rev}{b}3{/rev}{/b}
line 4
/3█
-----
[EVENT]:enter
# Title
line {rev}{b}3{/rev}{/b}
line 4
{rev}lines 3-5/6 83%{/rev}
-----
[EVENT]:char:q
";
    assert_eq!(screen.out(), want);
}

/// When searching and jumping with a global header,
/// the matched line is visible below the header, not hidden behind it.
#[test]
fn search_jump_with_global_header() {
    let content = "\
# Title
A
line 1
line 2
AB
line 4
line 5
AC
line 6
";
    let screen = run_test_screen(TestCase {
        screen_width: 20,
        screen_height: 5,
        content,
        options: Options {
            header: 1,
            ..Default::default()
        },
        events: vec![
            // Search by "A"
            key('/'),
            key('A'),
            enter(),
            // Jump around
            key('n'),
            key('n'),
            key('N'),
            key('q'),
        ],
        ..Default::default()
    });
    let want = "\
# Title
A
line 1
line 2
{rev}lines 1-4/9 44%{/rev}
-----
[EVENT]:char:/
# Title
A
line 1
line 2
/█
-----
[EVENT]:char:A
# Title
{rev}{b}A{/rev}{/b}
line 1
line 2
/A█
-----
[EVENT]:enter
# Title
{rev}{b}A{/rev}{/b}
line 1
line 2
{rev}lines 1-4/9 44%{/rev}
-----
[EVENT]:char:n
# Title
line 1
line 2
{rev}{b}A{/rev}{/b}B
{rev}lines 2-5/9 55%{/rev}
-----
[EVENT]:char:n
# Title
line 4
line 5
{rev}{b}A{/rev}{/b}C
{rev}lines 5-8/9 88%{/rev}
-----
[EVENT]:char:N
# Title
{rev}{b}A{/rev}{/b}B
line 4
line 5
{rev}lines 4-7/9 77%{/rev}
-----
[EVENT]:char:q
";
    assert_eq!(screen.out(), want);
}

/// When searching with a heading,
/// the matched line is visible below the sticky heading, not hidden behind it.
#[test]
fn search_with_heading() {
    let content = "\
# Section A
line 1
line 2
line 3
line 4
line 5
line 6
";
    let screen = run_test_screen(TestCase {
        screen_width: 20,
        screen_height: 4,
        content,
        options: Options {
            heading: heading_opts("^# ", 1),
            ..Default::default()
        },
        events: vec![key('/'), key('3'), enter(), key('q')],
        ..Default::default()
    });
    let want = "\
# Section A
line 1
line 2
{rev}lines 1-3/7 42%{/rev}
-----
[EVENT]:char:/
# Section A
line 1
line 2
/█
-----
[EVENT]:char:3
# Section A
line {rev}{b}3{/rev}{/b}
line 4
/3█
-----
[EVENT]:enter
# Section A
line {rev}{b}3{/rev}{/b}
line 4
{rev}lines 3-5/7 71%{/rev}
-----
[EVENT]:char:q
";
    assert_eq!(screen.out(), want);
}

/// When searching and jumping with a heading,
/// the matched line is visible below the sticky heading, not hidden behind it.
#[test]
fn search_jump_with_heading() {
    let content = "\
# Section 1
AAA
line 1
line 2
line 3
AAB
line 4
# Section 2
line 5
AAC
line 6
";
    let screen = run_test_screen(TestCase {
        screen_width: 20,
        screen_height: 5,
        content,
        options: Options {
            heading: heading_opts("^# ", 1),
            ..Default::default()
        },
        events: vec![
            // Search by "AA"
            key('/'),
            key('A'),
            key('A'),
            enter(),
            // Jump to next matches
            key('n'),
            key('n'),
            // Jump back
            key('N'),
            key('q'),
        ],
        ..Default::default()
    });
    let want = "\
# Section 1
AAA
line 1
line 2
{rev}lines 1-4/11 36%{/rev}
-----
[EVENT]:char:/
# Section 1
AAA
line 1
line 2
/█
-----
[EVENT]:char:A
# Section 1
{rev}{b}A{/rev}{/b}{rev}{line}{b}A{/rev}{/line}{/b}{rev}{line}{b}A{/rev}{/line}{/b}
line 1
line 2
/A█
-----
[EVENT]:char:A
# Section 1
{rev}{b}AA{/rev}{/b}A
line 1
line 2
/AA█
-----
[EVENT]:enter
# Section 1
{rev}{b}AA{/rev}{/b}A
line 1
line 2
{rev}lines 1-4/11 36%{/rev}
-----
[EVENT]:char:n
# Section 1
line 2
line 3
{rev}{b}AA{/rev}{/b}B
{rev}lines 3-6/11 54%{/rev}
-----
[EVENT]:char:n
# Section 1
# Section 2
line 5
{rev}{b}AA{/rev}{/b}C
{rev}lines 7-10/11 90%{/rev}
-----
[EVENT]:char:N
# Section 1
{rev}{b}AA{/rev}{/b}B
line 4
# Section 2
{rev}lines 5-8/11 72%{/rev}
-----
[EVENT]:char:q
";
    assert_eq!(screen.out(), want);
}

#[test]
fn search_with_heading_jump_back_one_line() {
    let content = "\
# Section A
127
128
129
130
131
132
133
134
";
    let screen = run_test_screen(TestCase {
        screen_width: 20,
        screen_height: 4,
        content,
        options: Options {
            heading: heading_opts("^# ", 1),
            ..Default::default()
        },
        events: vec![
            // Search by "13"
            key('/'),
            key('1'),
            key('3'),
            enter(),
            // Jump
            key('n'),
            key('n'),
            // Jump back
            key('N'),
            key('N'),
            key('q'),
        ],
        ..Default::default()
    });
    let want = "\
# Section A
127
128
{rev}lines 1-3/9 33%{/rev}
-----
[EVENT]:char:/
# Section A
127
128
/█
-----
[EVENT]:char:1
# Section A
{rev}{b}1{/rev}{/b}27
{rev}{line}{b}1{/rev}{/line}{/b}28
/1█
-----
[EVENT]:char:3
# Section A
{rev}{b}13{/rev}{/b}0
{rev}{line}{b}1{/rev}{/line}{/b}{line}{b}3{/line}{/b}1
/13█
-----
[EVENT]:enter
# Section A
{rev}{b}13{/rev}{/b}0
{rev}{line}{b}1{/rev}{/line}{/b}{line}{b}3{/line}{/b}1
{rev}lines 4-6/9 66%{/rev}
-----
[EVENT]:char:n
# Section A
{rev}{line}{b}1{/rev}{/line}{/b}{line}{b}3{/line}{/b}0
{rev}{b}13{/rev}{/b}1
{rev}lines 4-6/9 66%{/rev}
-----
[EVENT]:char:n
# Section A
{rev}{line}{b}1{/rev}{/line}{/b}{line}{b}3{/line}{/b}1
{rev}{b}13{/rev}{/b}2
{rev}lines 5-7/9 77%{/rev}
-----
[EVENT]:char:N
# Section A
{rev}{b}13{/rev}{/b}1
{rev}{line}{b}1{/rev}{/line}{/b}{line}{b}3{/line}{/b}2
{rev}lines 5-7/9 77%{/rev}
-----
[EVENT]:char:N
# Section A
{rev}{b}13{/rev}{/b}0
{rev}{line}{b}1{/rev}{/line}{/b}{line}{b}3{/line}{/b}1
{rev}lines 4-6/9 66%{/rev}
-----
[EVENT]:char:q
";
    assert_eq!(screen.out(), want);
}

#[test]
fn search_with_heading_multi_line() {
    let content = "\
# Section 1
description 1-1
description 1-2
line 1
line 2
A
line 3
line 4
line 5
AB
# Section 2
description 2-1
description 2-2
AC
line 6
line 7
line 8
";
    let screen = run_test_screen(TestCase {
        screen_width: 20,
        screen_height: 6,
        content,
        options: Options {
            heading: heading_opts("^# ", 3),
            ..Default::default()
        },
        events: vec![
            // Search with "A"
            key('/'),
            key('A'),
            enter(),
            // Jump
            key('n'),
            key('n'),
            key('q'),
        ],
        ..Default::default()
    });
    let want = "\
# Section 1
description 1-1
description 1-2
line 1
line 2
{rev}lines 1-5/17 29%{/rev}
-----
[EVENT]:char:/
# Section 1
description 1-1
description 1-2
line 1
line 2
/█
-----
[EVENT]:char:A
# Section 1
description 1-1
description 1-2
{rev}{b}A{/rev}{/b}
line 3
/A█
-----
[EVENT]:enter
# Section 1
description 1-1
description 1-2
{rev}{b}A{/rev}{/b}
line 3
{rev}lines 3-7/17 41%{/rev}
-----
[EVENT]:char:n
# Section 1
description 1-1
description 1-2
line 5
{rev}{b}A{/rev}{/b}B
{rev}lines 6-10/17 58%{/rev}
-----
[EVENT]:char:n
description 1-2
# Section 2
description 2-1
description 2-2
{rev}{b}A{/rev}{/b}C
{rev}lines 10-14/17 82%{/rev}
-----
[EVENT]:char:q
";
    assert_eq!(screen.out(), want);
}

#[test]
fn search_with_heading_containing_match() {
    let content = "\
# Section A1
AX
line 1
line 2
line 3
AY
line 4
# Section A2
line 5
AZ
line 6
line 7
";
    let screen = run_test_screen(TestCase {
        screen_width: 20,
        screen_height: 4,
        content,
        options: Options {
            heading: heading_opts("^# ", 1),
            ..Default::default()
        },
        events: vec![
            // Search with "A"
            key('/'),
            key('A'),
            enter(),
            // Jump
            key('n'),
            key('n'),
            key('n'),
            key('n'),
            key('N'),
            key('N'),
            key('q'),
        ],
        ..Default::default()
    });
    let want = "\
# Section A1
AX
line 1
{rev}lines 1-3/12 25%{/rev}
-----
[EVENT]:char:/
# Section A1
AX
line 1
/█
-----
[EVENT]:char:A
# Section {rev}{b}A{/rev}{/b}1
{rev}{line}{b}A{/rev}{/line}{/b}X
line 1
/A█
-----
[EVENT]:enter
# Section {rev}{b}A{/rev}{/b}1
{rev}{line}{b}A{/rev}{/line}{/b}X
line 1
{rev}lines 1-3/12 25%{/rev}
-----
[EVENT]:char:n
# Section {rev}{line}{b}A{/rev}{/line}{/b}1
{rev}{b}A{/rev}{/b}X
line 1
{rev}lines 1-3/12 25%{/rev}
-----
[EVENT]:char:n
# Section {rev}{line}{b}A{/rev}{/line}{/b}1
line 3
{rev}{b}A{/rev}{/b}Y
{rev}lines 4-6/12 50%{/rev}
-----
[EVENT]:char:n
# Section {rev}{line}{b}A{/rev}{/line}{/b}1
line 4
# Section {rev}{b}A{/rev}{/b}2
{rev}lines 6-8/12 66%{/rev}
-----
[EVENT]:char:n
# Section {rev}{line}{b}A{/rev}{/line}{/b}2
line 5
{rev}{b}A{/rev}{/b}Z
{rev}lines 8-10/12 83%{/rev}
-----
[EVENT]:char:N
# Section {rev}{b}A{/rev}{/b}2
line 5
{rev}{line}{b}A{/rev}{/line}{/b}Z
{rev}lines 8-10/12 83%{/rev}
-----
[EVENT]:char:N
# Section {rev}{line}{b}A{/rev}{/line}{/b}1
{rev}{b}A{/rev}{/b}Y
line 4
{rev}lines 5-7/12 58%{/rev}
-----
[EVENT]:char:q
";
    assert_eq!(screen.out(), want);
}
