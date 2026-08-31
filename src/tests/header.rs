use pretty_assertions::assert_eq;

use super::{TestCase, esc, key, run_test_screen};
use crate::options::Options;

#[test]
fn header_pinned_at_top() {
    let content = "\
HEADER 1
HEADER 2
line 3
line 4
line 5
line 6
";
    let screen = run_test_screen(TestCase {
        screen_width: 10,
        screen_height: 5,
        content,
        options: Options {
            header: 2,
            ..Default::default()
        },
        events: vec![key('q')],
        ..Default::default()
    });
    let want = "\
HEADER 1
HEADER 2
line 3
line 4
{rev} 1-4/6 66%{/rev}
-----
[EVENT]:char:q
";
    assert_eq!(screen.out(), want);
}

#[test]
fn header_stays_after_scroll_down() {
    let content = "\
HEADER 1
HEADER 2
line 3
line 4
line 5
line 6
";
    let screen = run_test_screen(TestCase {
        screen_width: 10,
        screen_height: 5,
        content,
        options: Options {
            header: 2,
            ..Default::default()
        },
        events: vec![key('j'), key('j'), key('q')],
        ..Default::default()
    });
    let want = "\
HEADER 1
HEADER 2
line 3
line 4
{rev} 1-4/6 66%{/rev}
-----
[EVENT]:char:j
HEADER 1
HEADER 2
line 4
line 5
{rev} 2-5/6 83%{/rev}
-----
[EVENT]:char:j
HEADER 1
HEADER 2
line 5
line 6
{rev}3-6/6 100%{/rev}
-----
[EVENT]:char:q
";
    assert_eq!(screen.out(), want);
}

#[test]
fn header_stays_after_scroll_up() {
    let content = "\
HEADER 1
HEADER 2
line 3
line 4
line 5
line 6
";
    let screen = run_test_screen(TestCase {
        screen_width: 10,
        screen_height: 5,
        content,
        options: Options {
            header: 2,
            ..Default::default()
        },
        events: vec![key('j'), key('j'), key('k'), key('q')],
        ..Default::default()
    });
    let want = "\
HEADER 1
HEADER 2
line 3
line 4
{rev} 1-4/6 66%{/rev}
-----
[EVENT]:char:j
HEADER 1
HEADER 2
line 4
line 5
{rev} 2-5/6 83%{/rev}
-----
[EVENT]:char:j
HEADER 1
HEADER 2
line 5
line 6
{rev}3-6/6 100%{/rev}
-----
[EVENT]:char:k
HEADER 1
HEADER 2
line 4
line 5
{rev} 2-5/6 83%{/rev}
-----
[EVENT]:char:q
";
    assert_eq!(screen.out(), want);
}

/// Scrolling up should not go above the header lines.
#[test]
fn cannot_scroll_above_header() {
    let content = "\
HEADER 1
HEADER 2
line 3
line 4
line 5
";
    let screen = run_test_screen(TestCase {
        screen_width: 10,
        screen_height: 5,
        content,
        options: Options {
            header: 2,
            ..Default::default()
        },
        events: vec![key('k'), key('k'), key('q')],
        ..Default::default()
    });
    let want = "\
HEADER 1
HEADER 2
line 3
line 4
{rev} 1-4/5 80%{/rev}
-----
[EVENT]:char:k
[EVENT]:char:k
[EVENT]:char:q
";
    assert_eq!(screen.out(), want);
}

/// 'g' should jump to the first non-header line.
#[test]
fn jump_to_top_respects_header() {
    let content = "\
HEADER
line 2
line 3
line 4
line 5
line 6
";
    let screen = run_test_screen(TestCase {
        screen_width: 10,
        screen_height: 5,
        content,
        options: Options {
            header: 1,
            ..Default::default()
        },
        events: vec![key('G'), key('g'), key('q')],
        ..Default::default()
    });
    let want = "\
HEADER
line 2
line 3
line 4
{rev} 1-4/6 66%{/rev}
-----
[EVENT]:char:G
HEADER
line 4
line 5
line 6
{rev}3-6/6 100%{/rev}
-----
[EVENT]:char:g
HEADER
line 2
line 3
line 4
{rev} 1-4/6 66%{/rev}
-----
[EVENT]:char:q
";
    assert_eq!(screen.out(), want);
}

/// 'G' should jump to the end while keeping the header.
#[test]
fn jump_to_end_with_header() {
    let content = "\
HEADER
line 2
line 3
line 4
line 5
line 6
";
    let screen = run_test_screen(TestCase {
        screen_width: 10,
        screen_height: 5,
        content,
        options: Options {
            header: 1,
            ..Default::default()
        },
        events: vec![key('G'), key('q')],
        ..Default::default()
    });
    let want = "\
HEADER
line 2
line 3
line 4
{rev} 1-4/6 66%{/rev}
-----
[EVENT]:char:G
HEADER
line 4
line 5
line 6
{rev}3-6/6 100%{/rev}
-----
[EVENT]:char:q
";
    assert_eq!(screen.out(), want);
}

/// header=0 should behave identically to no header.
#[test]
fn zero_header_is_noop() {
    let content = "\
line 1
line 2
line 3
line 4
line 5
";
    let screen = run_test_screen(TestCase {
        screen_width: 10,
        screen_height: 4,
        content,
        options: Options {
            header: 0,
            ..Default::default()
        },
        events: vec![key('q')],
        ..Default::default()
    });
    let want = "\
line 1
line 2
line 3
{rev} 1-3/5 60%{/rev}
-----
[EVENT]:char:q
";
    assert_eq!(screen.out(), want);
}

/// A `--header` wider than the document leaves no content below the header. Search input
/// still has to find a line to start from, and the header rows are that line.
#[test]
fn search_starts_in_the_header_when_it_covers_the_whole_document() {
    let content = "\
line 1
line 2
line 3
";
    let screen = run_test_screen(TestCase {
        screen_width: 20,
        screen_height: 6,
        content,
        options: Options {
            header: 5,
            ..Default::default()
        },
        events: vec![key('/'), esc(), key('q')],
        ..Default::default()
    });
    let want = "\
line 1
line 2
line 3
{rev}lines 1-3/3 100%{/rev}


-----
[EVENT]:char:/
line 1
line 2
line 3
/\u{2588}


-----
[EVENT]:esc
line 1
line 2
line 3
{rev}lines 1-3/3 100%{/rev}


-----
[EVENT]:char:q
";
    assert_eq!(screen.out(), want);
}
