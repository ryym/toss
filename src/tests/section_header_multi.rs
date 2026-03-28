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
            section: section_opts_n("^# ", 2),
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
";
    assert_eq!(screen.out(), want);
}

#[test]
fn header_switching() {
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
    });
    let want = "\
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
";
    assert_eq!(screen.out(), want);
}

#[test]
fn wrapped_header_switching() {
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
    });
    let want = "\
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
";
    assert_eq!(screen.out(), want);
}

/// When a line within the header block also matches the section pattern,
/// it should be treated as part of the current section's block, not as
/// a new section start.
#[test]
fn pattern_match_within_header_block() {
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
            section: section_opts_n("^#", 3),
            ..Default::default()
        },
        events: vec![key('j'), key('j'), key('q')],
        ..Default::default()
    });
    let want = "\
# Changelog

## 2.0.74

- Added LSP (Language Server Protocol) tool for code intelligence fe
:
-----
[EVENT]:char:j
# Changelog

## 2.0.74
- Added LSP (Language Server Protocol) tool for code intelligence fe>
atures like go-to-definition, find references, and hover documentati
:
-----
[EVENT]:char:j
# Changelog

## 2.0.74
atures like go-to-definition, find references, and hover documentati>
on
:
-----
[EVENT]:char:q
";
    assert_eq!(screen.out(), want);
}

#[test]
fn regression_wrapped_header_switching() {
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
            section: section_opts_n("^##", 3),
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
:
-----
[EVENT]:char:j

## 2.0.74

- Added LSP (Language Server Protocol) tool for code intelligence fe>
atures like go-to-definition, find references, and hover documentati>
on
:
-----
[EVENT]:char:j
## 2.0.74

- Added LSP (Language Server Protocol) tool for code intelligence fe>
atures like go-to-definition, find references, and hover documentati>
on
- Added `/terminal-setup` support for Kitty
:
-----
[EVENT]:char:j
## 2.0.74

- Added LSP (Language Server Protocol) tool for code intelligence fe>
atures like go-to-definition, find references, and hover documentati>
on

:
-----
[EVENT]:char:j
## 2.0.74

- Added LSP (Language Server Protocol) tool for code intelligence fe>
atures like go-to-definition, find references, and hover documentati>
on
## 2.0.73
:
-----
[EVENT]:char:j

- Added LSP (Language Server Protocol) tool for code intelligence fe>
atures like go-to-definition, find references, and hover documentati>
on
## 2.0.73

:
-----
[EVENT]:char:j
- Added LSP (Language Server Protocol) tool for code intelligence fe>
atures like go-to-definition, find references, and hover documentati>
on
## 2.0.73

- Added clickable `[Image #N]` links that open attached images in th
:
-----
[EVENT]:char:j
atures like go-to-definition, find references, and hover documentati>
on
## 2.0.73

- Added clickable `[Image #N]` links that open attached images in th>
e default viewer
:
-----
[EVENT]:char:j
on
## 2.0.73

- Added clickable `[Image #N]` links that open attached images in th>
e default viewer
- Added alt-y yank-pop to cycle through kill ring history after ctrl
:
-----
[EVENT]:char:j
## 2.0.73

- Added clickable `[Image #N]` links that open attached images in th>
e default viewer
- Added alt-y yank-pop to cycle through kill ring history after ctrl>
-y yank
:
-----
[EVENT]:char:q
";
    assert_eq!(screen.out(), want);
}

/// When section-header N equals the viewport content rows, the section header
/// fills the entire viewport. Scrolling down past the section start results
/// in a frozen display. Scrolling back up reveals pre-section content.
#[test]
fn section_header_fills_viewport() {
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
            section: section_opts_n("^# ", 4),
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
:
-----
[EVENT]:char:j
pre 2
# Section A
desc 1
desc 2
:
-----
[EVENT]:char:j
# Section A
desc 1
desc 2
body 1
:
-----
[EVENT]:char:j
# Section A
desc 1
desc 2
body 1
:
-----
[EVENT]:char:j
# Section A
desc 1
desc 2
body 1
:
-----
[EVENT]:char:k
# Section A
desc 1
desc 2
body 1
:
-----
[EVENT]:char:k
# Section A
desc 1
desc 2
body 1
:
-----
[EVENT]:char:k
pre 2
# Section A
desc 1
desc 2
:
-----
[EVENT]:char:k
pre 1
pre 2
# Section A
desc 1
:
-----
[EVENT]:char:q
";
    assert_eq!(screen.out(), want);
}

/// When section-header N exceeds the viewport content rows, only the first
/// viewport-height lines of the section header are visible (the rest are
/// truncated). The display freezes on the header while scrolling down.
#[test]
fn section_header_exceeds_viewport() {
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
            section: section_opts_n("^# ", 6),
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
:
-----
[EVENT]:char:j
pre 2
# Section A
desc 1
desc 2
:
-----
[EVENT]:char:j
# Section A
desc 1
desc 2
desc 3
:
-----
[EVENT]:char:j
# Section A
desc 1
desc 2
desc 3
:
-----
[EVENT]:char:j
# Section A
desc 1
desc 2
desc 3
:
-----
[EVENT]:char:k
# Section A
desc 1
desc 2
desc 3
:
-----
[EVENT]:char:k
# Section A
desc 1
desc 2
desc 3
:
-----
[EVENT]:char:k
pre 2
# Section A
desc 1
desc 2
:
-----
[EVENT]:char:k
pre 1
pre 2
# Section A
desc 1
:
-----
[EVENT]:char:q
";
    assert_eq!(screen.out(), want);
}
