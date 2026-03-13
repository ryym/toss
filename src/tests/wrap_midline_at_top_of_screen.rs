use pretty_assertions::assert_eq;

use super::{TestCase, key, run_test};

// When the top of the screen shows the middle of a wrapped line,
// visible wrap rows should still be soft-wrapped together.
// "abcdefghijk" wraps to "abcde" + "fghij" + "k" at width 5.
#[test]
fn run() {
    let out = run_test(TestCase {
        screen_width: 5,
        screen_height: 3,
        content: "\
abcdefghijk
end",
        events: vec![key('j'), key('q')],
    });
    // Initial: abcde>, fghij>, k
    // j: fghij>, k, end
    // Even though "abcde" is off-screen, "fghij" and "k" should be
    // soft-wrapped together.
    assert_eq!(
        out,
        "\
abcde>
fghij>
k
-----
[EVENT]:char:j
fghij>
k
end
-----
[EVENT]:char:q
"
    );
}
