use pretty_assertions::assert_eq;

use super::{TestCase, key, run_test_screen};
use crate::options::{HeadingOptions, Options};

fn heading_opts_n(pattern: &str, num_lines: usize) -> Option<HeadingOptions> {
    Some(HeadingOptions {
        pattern: regex::Regex::new(pattern).unwrap(),
        num_lines,
    })
}

#[test]
fn sticky_heading() {
    let content = "\
# Section A
description A
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
            heading: heading_opts_n("^# ", 2),
            ..Default::default()
        },
        events: vec![key('j'), key('j'), key('q')],
        ..Default::default()
    });
    let want = "\
# Section A
description A
line 1
line 2
line 3
{rev}lines 1-5/7 71%{/rev}
-----
[EVENT]:char:j
# Section A
description A
line 2
line 3
line 4
{rev}lines 2-6/7 85%{/rev}
-----
[EVENT]:char:j
# Section A
description A
line 3
line 4
line 5
{rev}lines 3-7/7 100%{/rev}
-----
[EVENT]:char:q
";
    assert_eq!(screen.out(), want);
}

#[test]
fn heading_switching() {
    let content = "\
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
line 10
";
    let screen = run_test_screen(TestCase {
        screen_width: 20,
        screen_height: 6,
        content,
        options: Options {
            heading: heading_opts_n("^# ", 2),
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
    });
    let want = "\
# Section A
description A
line 1
line 2
line 3
{rev}lines 1-5/13 38%{/rev}
-----
[EVENT]:char:j
# Section A
description A
line 2
line 3
line 4
{rev}lines 2-6/13 46%{/rev}
-----
[EVENT]:char:j
# Section A
description A
line 3
line 4
# Section B
{rev}lines 3-7/13 53%{/rev}
-----
[EVENT]:char:j
# Section A
description A
line 4
# Section B
description B
{rev}lines 4-8/13 61%{/rev}
-----
[EVENT]:char:j
# Section A
description A
# Section B
description B
line 5
{rev}lines 5-9/13 69%{/rev}
-----
[EVENT]:char:j
description A
# Section B
description B
line 5
line 6
{rev}lines 6-10/13 76%{/rev}
-----
[EVENT]:char:j
# Section B
description B
line 5
line 6
line 7
{rev}lines 7-11/13 84%{/rev}
-----
[EVENT]:char:q
";
    assert_eq!(screen.out(), want);
}

#[test]
fn wrapped_heading_switching() {
    let content = "\
# abcde
012345678
line 1
line 2
line 3
# fghig
012345678
line 4
line 5
line 6
";
    let screen = run_test_screen(TestCase {
        screen_width: 7,
        screen_height: 6,
        content,
        options: Options {
            heading: heading_opts_n("^# ", 2),
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
    });
    let want = "\
# abcde
0123456>
78
line 1
line 2
{rev}/10 40%{/rev}
-----
[EVENT]:char:j
# abcde
0123456>
78
line 2
line 3
{rev}/10 50%{/rev}
-----
[EVENT]:char:j
# abcde
0123456>
78
line 3
# fghig
{rev}/10 60%{/rev}
-----
[EVENT]:char:j
# abcde
0123456>
78
# fghig
0123456
{rev}/10 70%{/rev}
-----
[EVENT]:char:j
0123456>
78
# fghig
0123456>
78
{rev}/10 70%{/rev}
-----
[EVENT]:char:j
78
# fghig
0123456>
78
line 4
{rev}/10 80%{/rev}
-----
[EVENT]:char:j
# fghig
0123456>
78
line 4
line 5
{rev}/10 90%{/rev}
-----
[EVENT]:char:q
";
    assert_eq!(screen.out(), want);
}

/// When a line within the heading block also matches the section pattern,
/// it should be treated as part of the current section's block, not as
/// a new section start.
#[test]
fn pattern_match_within_heading_block() {
    let content = "\
# Changelog

## 2.0.74

- Added LSP (Language Server Protocol) tool for code intelligence features like go-to-definition, find references, and hover documentation
- Added `/terminal-setup` support for Kitty, Alacritty, Zed, and Warp terminals
- Added ctrl+t shortcut in `/theme` to toggle syntax highlighting on/off
";
    let screen = run_test_screen(TestCase {
        screen_width: 68,
        screen_height: 6,
        content,
        options: Options {
            heading: heading_opts_n("^#", 3),
            ..Default::default()
        },
        events: vec![key('j'), key('j'), key('j'), key('q')],
        ..Default::default()
    });
    let want = "\
# Changelog

## 2.0.74

- Added LSP (Language Server Protocol) tool for code intelligence fe
{rev}lines 1-5/7 71%{/rev}
-----
[EVENT]:char:j

## 2.0.74

- Added LSP (Language Server Protocol) tool for code intelligence fe>
atures like go-to-definition, find references, and hover documentati
{rev}lines 2-5/7 71%{/rev}
-----
[EVENT]:char:j
## 2.0.74

- Added LSP (Language Server Protocol) tool for code intelligence fe>
atures like go-to-definition, find references, and hover documentati
on
{rev}lines 3-5/7 71%{/rev}
-----
[EVENT]:char:j
## 2.0.74

- Added LSP (Language Server Protocol) tool for code intelligence fe>
atures like go-to-definition, find references, and hover documentati
- Added `/terminal-setup` support for Kitty, Alacritty, Zed, and War
{rev}lines 4-6/7 85%{/rev}
-----
[EVENT]:char:q
";
    assert_eq!(screen.out(), want);
}

#[test]
fn regression_wrapped_heading_switching() {
    let content = "\
# Changelog

## 2.0.74

- Added LSP (Language Server Protocol) tool for code intelligence features like go-to-definition, find references, and hover documentation
- Added `/terminal-setup` support for Kitty

## 2.0.73

- Added clickable `[Image #N]` links that open attached images in the default viewer
- Added alt-y yank-pop to cycle through kill ring history after ctrl-y yank
- Added search filtering to the plugin discover screen (type to filter by name, description, or marketplace)
";
    let screen = run_test_screen(TestCase {
        screen_width: 68,
        screen_height: 7,
        content,
        options: Options {
            heading: heading_opts_n("^##", 3),
            ..Default::default()
        },
        events: vec![
            key('j'),
            key('j'),
            key('j'),
            key('j'),
            key('j'),
            key('j'),
            key('j'),
            key('j'),
            key('j'),
            key('q'),
        ],
        ..Default::default()
    });
    let want = "\
# Changelog

## 2.0.74

- Added LSP (Language Server Protocol) tool for code intelligence fe>
atures like go-to-definition, find references, and hover documentati
{rev}lines 1-5/12 41%{/rev}
-----
[EVENT]:char:j

## 2.0.74

- Added LSP (Language Server Protocol) tool for code intelligence fe>
atures like go-to-definition, find references, and hover documentati>
on
{rev}lines 2-5/12 41%{/rev}
-----
[EVENT]:char:j
## 2.0.74

- Added LSP (Language Server Protocol) tool for code intelligence fe>
atures like go-to-definition, find references, and hover documentati>
on
- Added `/terminal-setup` support for Kitty
{rev}lines 3-6/12 50%{/rev}
-----
[EVENT]:char:j
## 2.0.74

- Added LSP (Language Server Protocol) tool for code intelligence fe>
atures like go-to-definition, find references, and hover documentati>
on

{rev}lines 4-7/12 58%{/rev}
-----
[EVENT]:char:j
## 2.0.74

- Added LSP (Language Server Protocol) tool for code intelligence fe>
atures like go-to-definition, find references, and hover documentati>
on
## 2.0.73
{rev}lines 5-8/12 66%{/rev}
-----
[EVENT]:char:j

- Added LSP (Language Server Protocol) tool for code intelligence fe>
atures like go-to-definition, find references, and hover documentati>
on
## 2.0.73

{rev}lines 5-9/12 75%{/rev}
-----
[EVENT]:char:j
- Added LSP (Language Server Protocol) tool for code intelligence fe>
atures like go-to-definition, find references, and hover documentati>
on
## 2.0.73

- Added clickable `[Image #N]` links that open attached images in th
{rev}lines 5-10/12 83%{/rev}
-----
[EVENT]:char:j
atures like go-to-definition, find references, and hover documentati>
on
## 2.0.73

- Added clickable `[Image #N]` links that open attached images in th>
e default viewer
{rev}lines 6-10/12 83%{/rev}
-----
[EVENT]:char:j
on
## 2.0.73

- Added clickable `[Image #N]` links that open attached images in th>
e default viewer
- Added alt-y yank-pop to cycle through kill ring history after ctrl
{rev}lines 7-11/12 91%{/rev}
-----
[EVENT]:char:j
## 2.0.73

- Added clickable `[Image #N]` links that open attached images in th>
e default viewer
- Added alt-y yank-pop to cycle through kill ring history after ctrl>
-y yank
{rev}lines 8-11/12 91%{/rev}
-----
[EVENT]:char:q
";
    assert_eq!(screen.out(), want);
}

/// When heading-lines N equals the viewport content rows, the heading
/// fills the entire viewport. Scrolling down past the section start results
/// in a frozen display. Scrolling back up reveals pre-section content.
#[test]
fn heading_fills_viewport() {
    let content = "\
pre 1
pre 2
# Section A
desc 1
desc 2
body 1
body 2
body 3
body 4
body 5
";
    let screen = run_test_screen(TestCase {
        screen_width: 20,
        screen_height: 5,
        content,
        options: Options {
            heading: heading_opts_n("^# ", 4),
            ..Default::default()
        },
        events: vec![
            key('j'),
            key('j'),
            key('j'),
            key('j'),
            key('k'),
            key('k'),
            key('k'),
            key('k'),
            key('q'),
        ],
        ..Default::default()
    });
    let want = "\
pre 1
pre 2
# Section A
desc 1
{rev}lines 1-4/10 40%{/rev}
-----
[EVENT]:char:j
pre 2
# Section A
desc 1
desc 2
{rev}lines 2-5/10 50%{/rev}
-----
[EVENT]:char:j
# Section A
desc 1
desc 2
body 1
{rev}lines 3-6/10 60%{/rev}
-----
[EVENT]:char:j
# Section A
desc 1
desc 2
body 2
{rev}lines 4-7/10 70%{/rev}
-----
[EVENT]:char:j
# Section A
desc 1
desc 2
body 3
{rev}lines 5-8/10 80%{/rev}
-----
[EVENT]:char:k
# Section A
desc 1
desc 2
body 2
{rev}lines 4-7/10 70%{/rev}
-----
[EVENT]:char:k
# Section A
desc 1
desc 2
body 1
{rev}lines 3-6/10 60%{/rev}
-----
[EVENT]:char:k
pre 2
# Section A
desc 1
desc 2
{rev}lines 2-5/10 50%{/rev}
-----
[EVENT]:char:k
pre 1
pre 2
# Section A
desc 1
{rev}lines 1-4/10 40%{/rev}
-----
[EVENT]:char:q
";
    assert_eq!(screen.out(), want);
}

/// When heading-lines N exceeds the viewport content rows, only the first
/// viewport-height lines of the heading are visible (the rest are
/// truncated). The display freezes on the heading while scrolling down.
#[test]
fn heading_exceeds_viewport() {
    let content = "\
pre 1
pre 2
# Section A
desc 1
desc 2
desc 3
desc 4
desc 5
body 1
body 2
body 3
body 4
";
    let screen = run_test_screen(TestCase {
        screen_width: 20,
        screen_height: 5,
        content,
        options: Options {
            heading: heading_opts_n("^# ", 6),
            ..Default::default()
        },
        events: vec![
            key('j'),
            key('j'),
            key('j'),
            key('j'),
            key('k'),
            key('k'),
            key('k'),
            key('k'),
            key('q'),
        ],
        ..Default::default()
    });
    let want = "\
pre 1
pre 2
# Section A
desc 1
{rev}lines 1-4/12 33%{/rev}
-----
[EVENT]:char:j
pre 2
# Section A
desc 1
desc 2
{rev}lines 2-5/12 41%{/rev}
-----
[EVENT]:char:j
# Section A
desc 1
desc 2
desc 3
{rev}lines 3-6/12 50%{/rev}
-----
[EVENT]:char:j
# Section A
desc 1
desc 2
desc 4
{rev}lines 4-7/12 58%{/rev}
-----
[EVENT]:char:j
# Section A
desc 1
desc 2
desc 5
{rev}lines 5-8/12 66%{/rev}
-----
[EVENT]:char:k
# Section A
desc 1
desc 2
desc 4
{rev}lines 4-7/12 58%{/rev}
-----
[EVENT]:char:k
# Section A
desc 1
desc 2
desc 3
{rev}lines 3-6/12 50%{/rev}
-----
[EVENT]:char:k
pre 2
# Section A
desc 1
desc 2
{rev}lines 2-5/12 41%{/rev}
-----
[EVENT]:char:k
pre 1
pre 2
# Section A
desc 1
{rev}lines 1-4/12 33%{/rev}
-----
[EVENT]:char:q
";
    assert_eq!(screen.out(), want);
}
