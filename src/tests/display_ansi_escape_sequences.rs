use pretty_assertions::assert_eq;

use super::{TestCase, key, run_test};

// Lines with ANSI escape sequences should display correctly.
// Escape sequences have zero display width, so wrapping is based on
// visible characters only. The raw escape sequences are preserved in output.
#[test]
fn run() {
    // "\x1b[1m" = bold, "\x1b[0m" = reset, "\x1b[31m" = red
    // Line 1 visible: "Hello" (5 cols, fits in width 5)
    // Line 2 visible: "abcdefgh" (8 cols, wraps to "abcde" + "fgh" at width 5)
    // Line 3 visible: "end" (3 cols)
    let out = run_test(TestCase {
        screen_width: 5,
        screen_height: 4,
        content: "\
\x1b[1mHello\x1b[0m
\x1b[31mabcde\x1b[0mfgh
end",
        events: vec![key('q')],
    });
    assert_eq!(
        out,
        "\
\x1b[1mHello\x1b[0m
\x1b[31mabcde\x1b[0m>
fgh
end
-----
[EVENT]:char:q
"
    );
}
