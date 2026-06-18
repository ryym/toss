use pretty_assertions::assert_eq;

use super::{TestCase, key, run_test_screen};

#[test]
fn open_and_quit() {
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
        events: vec![key('q')],
        ..Default::default()
    });
    let want = "\
line 1
line 2
line 3
 1-3/5 60%
-----
[EVENT]:char:q
";
    assert_eq!(screen.out(), want);
}

// Lines with ANSI escape sequences should display correctly.
// Escape sequences have zero display width, so wrapping is based on
// visible characters only. The raw escape sequences are preserved in output.
#[test]
fn ansi_escape_sequences() {
    // "\x1b[1m" = bold, "\x1b[0m" = reset, "\x1b[31m" = red
    // Line 1 visible: "Hello" (5 cols, fits in width 5)
    // Line 2 visible: "abcdefgh" (8 cols, wraps to "abcde" + "fgh" at width 5)
    // Line 3 visible: "end" (3 cols)
    let content = "\
\x1b[1mHello\x1b[0m
\x1b[31mabcde\x1b[0mfgh
end
";
    let screen = run_test_screen(TestCase {
        screen_width: 5,
        screen_height: 5,
        content,
        events: vec![key('q')],
        ..Default::default()
    });
    let want = "\
{b}Hello{reset}
{red}abcde{reset}>
fgh
end
 100%
-----
[EVENT]:char:q
";
    assert_eq!(screen.out(), want);
}

// Verify the status line appears on the last row and survives scroll operations.
#[test]
fn status_line() {
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
        events: vec![key('j'), key('k'), key('q')],
        ..Default::default()
    });
    let want = "\
line 1
line 2
line 3
 1-3/5 60%
-----
[EVENT]:char:j
line 2
line 3
line 4
 2-4/5 80%
-----
[EVENT]:char:k
line 1
line 2
line 3
 1-3/5 60%
-----
[EVENT]:char:q
";
    assert_eq!(screen.out(), want);
}
