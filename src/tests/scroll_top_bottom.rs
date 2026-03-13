use pretty_assertions::assert_eq;

use super::{TestCase, key, run_test_screen};

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
line 5
line 6
line 7
line 8",
        events: vec![key('G'), key('g'), key('q')],
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
[EVENT]:char:G
line 6
line 7
line 8
:
-----
[EVENT]:char:g
line 1
line 2
line 3
:
-----
[EVENT]:char:q
"
    );
}
