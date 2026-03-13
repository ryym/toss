use pretty_assertions::assert_eq;

use super::{TestCase, key, run_test_screen};

// Scrolling up to reveal a new wrap row that has the same line as rows below.
#[test]
fn run() {
    let out = run_test_screen(TestCase {
        screen_width: 5,
        screen_height: 4,
        content: "\
xx
abcdefgh
yy",
        events: vec![key('j'), key('j'), key('k'), key('q')],
    })
    .out();
    // "abcdefgh" wraps to "abcde" + "fgh"
    // Initial: xx, abcde>, fgh
    // j: abcde>, fgh, yy
    // j: can't scroll (yy is last)
    // k from [abcde>, fgh, yy]: back to [xx, abcde>, fgh]
    assert_eq!(
        out,
        "\
xx
abcde>
fgh
:
-----
[EVENT]:char:j
abcde>
fgh
yy
:
-----
[EVENT]:char:j
[EVENT]:char:k
xx
abcde>
fgh
:
-----
[EVENT]:char:q
"
    );
}
