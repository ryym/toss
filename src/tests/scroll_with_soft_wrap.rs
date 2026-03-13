use pretty_assertions::assert_eq;

use super::{TestCase, key, run_test_screen};

// "abcdefgh" wraps to "abcde" + "fgh" at width 5.
// Initial display shows them with soft wrap marker '>'.
#[test]
fn run() {
    let out = run_test_screen(TestCase {
        screen_width: 5,
        screen_height: 4,
        content: "\
short
abcdefgh
end",
        events: vec![key('j'), key('j'), key('k'), key('q')],
    })
    .out();
    assert_eq!(
        out,
        "\
short
abcde>
fgh
:
-----
[EVENT]:char:j
abcde>
fgh
end
:
-----
[EVENT]:char:j
[EVENT]:char:k
short
abcde>
fgh
:
-----
[EVENT]:char:q
"
    );
}
