use pretty_assertions::assert_eq;

use super::{TestCase, key, run_test_screen};

// When scrolling down reveals a new wrap row, the entire visible
// portion of that line is redrawn to maintain soft wraps.
// Line "aaabbbccc" wraps to 3 rows at width 3.
#[test]
fn run() {
    let out = run_test_screen(TestCase {
        screen_width: 3,
        screen_height: 5,
        content: "\
xx
aaabbbccc
yy",
        events: vec![key('j'), key('j'), key('q')],
    })
    .out();
    // Initial: xx, aaa>, bbb>, ccc
    // After j: aaa>, bbb>, ccc, yy
    // After j: can't scroll (yy is last)
    assert_eq!(
        out,
        "\
xx
aaa>
bbb>
ccc
:
-----
[EVENT]:char:j
aaa>
bbb>
ccc
yy
:
-----
[EVENT]:char:j
[EVENT]:char:q
"
    );
}
