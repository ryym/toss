use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use pretty_assertions::assert_eq;

use super::{TestCase, key, run_test_screen};
use crate::options::{HeadingOptions, Options};

fn enter() -> Event {
    Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
}

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
        events: vec![key('/'), key('3'), enter()],
        ..Default::default()
    });
    let want = "\
# Title
line 1
line 2
:
-----
[EVENT]:char:/
# Title
line 1
line 2
/█
-----
[EVENT]:char:3
# Title
line {reverse}{bold}3{/reverse}{/bold}
line 4
/3█
-----
[EVENT]:enter
# Title
line {reverse}{bold}3{/reverse}{/bold}
line 4
:
-----
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
        ],
        ..Default::default()
    });
    let want = "\
# Title
A
line 1
line 2
:
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
{reverse}{bold}A{/reverse}{/bold}
line 1
line 2
/A█
-----
[EVENT]:enter
# Title
{reverse}{bold}A{/reverse}{/bold}
line 1
line 2
:
-----
[EVENT]:char:n
# Title
{reverse}{bold}A{/reverse}{/bold}B
line 4
line 5
:
-----
[EVENT]:char:n
# Title
line 5
{reverse}{bold}A{/reverse}{/bold}C
line 6
:
-----
[EVENT]:char:N
# Title
{reverse}{bold}A{/reverse}{/bold}B
line 4
line 5
:
-----
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
        events: vec![key('/'), key('3'), enter()],
        ..Default::default()
    });
    let want = "\
# Section A
line 1
line 2
:
-----
[EVENT]:char:/
# Section A
line 1
line 2
/█
-----
[EVENT]:char:3
# Section A
line {reverse}{bold}3{/reverse}{/bold}
line 4
/3█
-----
[EVENT]:enter
# Section A
line {reverse}{bold}3{/reverse}{/bold}
line 4
:
-----
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
        ],
        ..Default::default()
    });
    let want = "\
# Section 1
AAA
line 1
line 2
:
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
{reverse}{bold}A{/reverse}{/bold}{reverse}{underline}{bold}A{/reverse}{/underline}{/bold}{reverse}{underline}{bold}A{/reverse}{/underline}{/bold}
line 1
line 2
/A█
-----
[EVENT]:char:A
# Section 1
{reverse}{bold}AA{/reverse}{/bold}A
line 1
line 2
/AA█
-----
[EVENT]:enter
# Section 1
{reverse}{bold}AA{/reverse}{/bold}A
line 1
line 2
:
-----
[EVENT]:char:n
# Section 1
{reverse}{bold}AA{/reverse}{/bold}B
line 4
# Section 2
:
-----
[EVENT]:char:n
# Section 2
line 5
{reverse}{bold}AA{/reverse}{/bold}C
line 6
:
-----
[EVENT]:char:N
# Section 1
{reverse}{bold}AA{/reverse}{/bold}B
line 4
# Section 2
:
-----
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
        ],
        ..Default::default()
    });
    let want = "\
# Section A
127
128
:
-----
[EVENT]:char:/
# Section A
127
128
/█
-----
[EVENT]:char:1
# Section A
{reverse}{bold}1{/reverse}{/bold}27
{reverse}{underline}{bold}1{/reverse}{/underline}{/bold}28
/1█
-----
[EVENT]:char:3
# Section A
{reverse}{bold}13{/reverse}{/bold}0
{reverse}{underline}{bold}1{/reverse}{/underline}{/bold}{underline}{bold}3{/underline}{/bold}1
/13█
-----
[EVENT]:enter
# Section A
{reverse}{bold}13{/reverse}{/bold}0
{reverse}{underline}{bold}1{/reverse}{/underline}{/bold}{underline}{bold}3{/underline}{/bold}1
:
-----
[EVENT]:char:n
# Section A
{reverse}{bold}13{/reverse}{/bold}1
{reverse}{underline}{bold}1{/reverse}{/underline}{/bold}{underline}{bold}3{/underline}{/bold}2
:
-----
[EVENT]:char:n
# Section A
{reverse}{bold}13{/reverse}{/bold}2
{reverse}{underline}{bold}1{/reverse}{/underline}{/bold}{underline}{bold}3{/underline}{/bold}3
:
-----
[EVENT]:char:N
# Section A
{reverse}{bold}13{/reverse}{/bold}1
{reverse}{underline}{bold}1{/reverse}{/underline}{/bold}{underline}{bold}3{/underline}{/bold}2
:
-----
[EVENT]:char:N
# Section A
{reverse}{bold}13{/reverse}{/bold}0
{reverse}{underline}{bold}1{/reverse}{/underline}{/bold}{underline}{bold}3{/underline}{/bold}1
:
-----
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
        ],
        ..Default::default()
    });
    let want = "\
# Section 1
description 1-1
description 1-2
line 1
line 2
:
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
{reverse}{bold}A{/reverse}{/bold}
line 3
/A█
-----
[EVENT]:enter
# Section 1
description 1-1
description 1-2
{reverse}{bold}A{/reverse}{/bold}
line 3
:
-----
[EVENT]:char:n
# Section 1
description 1-1
description 1-2
{reverse}{bold}A{/reverse}{/bold}B
# Section 2
:
-----
[EVENT]:char:n
# Section 2
description 2-1
description 2-2
{reverse}{bold}A{/reverse}{/bold}C
line 6
:
-----
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
        ],
        ..Default::default()
    });
    let want = "\
# Section A1
AX
line 1
:
-----
[EVENT]:char:/
# Section A1
AX
line 1
/█
-----
[EVENT]:char:A
# Section {reverse}{bold}A{/reverse}{/bold}1
{reverse}{underline}{bold}A{/reverse}{/underline}{/bold}X
line 1
/A█
-----
[EVENT]:enter
# Section {reverse}{bold}A{/reverse}{/bold}1
{reverse}{underline}{bold}A{/reverse}{/underline}{/bold}X
line 1
:
-----
[EVENT]:char:n
# Section {reverse}{underline}{bold}A{/reverse}{/underline}{/bold}1
{reverse}{bold}A{/reverse}{/bold}X
line 1
:
-----
[EVENT]:char:n
# Section {reverse}{underline}{bold}A{/reverse}{/underline}{/bold}1
{reverse}{bold}A{/reverse}{/bold}Y
line 4
:
-----
[EVENT]:char:n
# Section {reverse}{bold}A{/reverse}{/bold}2
line 5
{reverse}{underline}{bold}A{/reverse}{/underline}{/bold}Z
:
-----
[EVENT]:char:n
# Section {reverse}{underline}{bold}A{/reverse}{/underline}{/bold}2
{reverse}{bold}A{/reverse}{/bold}Z
line 6
:
-----
[EVENT]:char:N
# Section {reverse}{bold}A{/reverse}{/bold}2
line 5
{reverse}{underline}{bold}A{/reverse}{/underline}{/bold}Z
:
-----
[EVENT]:char:N
# Section {reverse}{underline}{bold}A{/reverse}{/underline}{/bold}1
{reverse}{bold}A{/reverse}{/bold}Y
line 4
:
-----
";
    assert_eq!(screen.out(), want);
}
