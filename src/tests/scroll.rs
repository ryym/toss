use pretty_assertions::assert_eq;

use super::{TestCase, key, run_test_screen};

#[test]
fn up_down() {
    let out = run_test_screen(TestCase {
        screen_width: 10,
        screen_height: 4,
        content: "\
line 1
line 2
line 3
line 4
line 5",
        events: vec![key('j'), key('j'), key('k'), key('q')],
        ..Default::default()
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
[EVENT]:char:j
line 3
line 4
line 5
:
-----
[EVENT]:char:k
line 2
line 3
line 4
:
-----
[EVENT]:char:q
"
    );
}

#[test]
fn half_page() {
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
        ..Default::default()
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

#[test]
fn top_bottom() {
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
        ..Default::default()
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

#[test]
fn cannot_past_boundaries() {
    let out = run_test_screen(TestCase {
        screen_width: 10,
        screen_height: 4,
        content: "\
line 1
line 2
line 3",
        events: vec![key('k'), key('j'), key('q')],
        ..Default::default()
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
[EVENT]:char:k
[EVENT]:char:j
[EVENT]:char:q
"
    );
}
