use pretty_assertions::assert_eq;

use super::{TestCase, key, run_test};

#[test]
fn run() {
    let out = run_test(TestCase {
        screen_width: 5,
        screen_height: 4,
        content: "\
line1
line2
abcdefgh",
        events: vec![key('G'), key('g'), key('q')],
    });
    // "abcdefgh" wraps to "abcde" + "fgh"
    // Initial: line1, line2, abcde (fgh off-screen, no soft wrap)
    // G: line2, abcde>, fgh
    // g: line1, line2, abcde (fgh off-screen again)
    assert_eq!(
        out,
        "\
line1
line2
abcde
:
-----
[EVENT]:char:G
line2
abcde>
fgh
:
-----
[EVENT]:char:g
line1
line2
abcde
:
-----
[EVENT]:char:q
"
    );
}
