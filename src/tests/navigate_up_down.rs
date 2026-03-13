use pretty_assertions::assert_eq;

use super::{TestCase, key, run_test};

#[test]
fn run() {
    let out = run_test(TestCase {
        screen_width: 10,
        screen_height: 3,
        content: "\
line 1
line 2
line 3
line 4
line 5",
        events: vec![key('j'), key('j'), key('k'), key('q')],
    });
    assert_eq!(
        out,
        "\
line 1
line 2
line 3
-----
[EVENT]:char:j
line 2
line 3
line 4
-----
[EVENT]:char:j
line 3
line 4
line 5
-----
[EVENT]:char:k
line 2
line 3
line 4
-----
[EVENT]:char:q
"
    );
}
