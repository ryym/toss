use pretty_assertions::assert_eq;

use super::{TestCase, key, run_test};

// When scrolling down reveals a new wrap row, the entire visible
// portion of that line is redrawn to maintain soft wraps.
// Line "aaabbbccc" wraps to 3 rows at width 3.
#[test]
fn run() {
    let out = run_test(TestCase {
        screen_width: 3,
        screen_height: 4,
        content: "\
xx
aaabbbccc
yy",
        events: vec![key('j'), key('j'), key('q')],
    });
    // Initial: xx, aaa>, bbb>, ccc (line 1 wraps to 3 rows)
    // After j: aaa>, bbb>, ccc, yy
    // After j: bbb>, ccc, yy, (empty) - but yy is last, can't scroll
    assert_eq!(
        out,
        "\
xx
aaa>
bbb>
ccc
-----
[EVENT]:char:j
aaa>
bbb>
ccc
yy
-----
[EVENT]:char:j
[EVENT]:char:q
"
    );
}
