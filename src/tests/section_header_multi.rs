use pretty_assertions::assert_eq;

use super::{TestCase, key, run_test_screen};
use crate::options::{Options, SectionOptions};

fn section_opts_n(pattern: &str, header_lines: usize) -> Option<SectionOptions> {
    Some(SectionOptions {
        pattern: regex::Regex::new(pattern).unwrap(),
        header_lines,
    })
}

#[test]
fn sticky_header() {
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

#[test]
fn header_switching() {
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

#[test]
fn wrapped_header_switching() {
    let out = run_test_screen(TestCase {
        screen_width: 7,
        screen_height: 6,
        content: "\
# abcde
012345678
line 1
line 2
line 3
# fghig
012345678
line 4
line 5
line 6",
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
# abcde
0123456>
78
line 1
line 2
:
-----
[EVENT]:char:j
# abcde
0123456>
78
line 2
line 3
:
-----
[EVENT]:char:j
# abcde
0123456>
78
line 3
# fghig
:
-----
[EVENT]:char:j
# abcde
0123456>
78
# fghig
0123456
:
-----
[EVENT]:char:j
0123456>
78
# fghig
0123456>
78
:
-----
[EVENT]:char:j
78
# fghig
0123456>
78
line 4
:
-----
[EVENT]:char:j
# fghig
0123456>
78
line 4
line 5
:
-----
[EVENT]:char:q
"
    );
}
