use pretty_assertions::assert_eq;

use super::{TestCase, key, run_test_screen};

// Verify the status line appears on the last row and survives scroll operations.
#[test]
fn run() {
    let out = run_test_screen(TestCase {
        screen_width: 10,
        screen_height: 4,
        content: "\
line 1
line 2
line 3
line 4
line 5",
        events: vec![key('j'), key('k'), key('q')],
    })
    .out();
    assert_eq!(
        out,
        "\
line 1
line 2
line 3
:
-----
[EVENT]:char:j
line 2
line 3
line 4
:
-----
[EVENT]:char:k
line 1
line 2
line 3
:
-----
[EVENT]:char:q
"
    );
}
