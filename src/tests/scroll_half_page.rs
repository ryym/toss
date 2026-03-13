use pretty_assertions::assert_eq;

use super::{TestCase, key, run_test_screen};

#[test]
fn run() {
    let out = run_test_screen(TestCase {
        screen_width: 10,
        screen_height: 5,
        content: "\
line 1
line 2
line 3
line 4
line 5
line 6
line 7
line 8",
        events: vec![key('d'), key('u'), key('q')],
    })
    .out();
    assert_eq!(
        out,
        "\
line 1
line 2
line 3
line 4
:
-----
[EVENT]:char:d
line 3
line 4
line 5
line 6
:
-----
[EVENT]:char:u
line 1
line 2
line 3
line 4
:
-----
[EVENT]:char:q
"
    );
}
