use pretty_assertions::assert_eq;

use super::{TestCase, key, run_test_screen};

#[test]
fn up_down() {
    let content = "\
line 1
line 2
line 3
line 4
line 5
";
    let screen = run_test_screen(TestCase {
        screen_width: 10,
        screen_height: 4,
        content,
        events: vec![key('j'), key('j'), key('k'), key('q')],
        ..Default::default()
    });
    let want = "\
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
";
    assert_eq!(screen.out(), want);
}

#[test]
fn half_page() {
    let content = "\
line 1
line 2
line 3
line 4
line 5
line 6
line 7
line 8
";
    let screen = run_test_screen(TestCase {
        screen_width: 10,
        screen_height: 5,
        content,
        events: vec![key('d'), key('u'), key('q')],
        ..Default::default()
    });
    let want = "\
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
";
    assert_eq!(screen.out(), want);
}

#[test]
fn top_bottom() {
    let content = "\
line 1
line 2
line 3
line 4
line 5
line 6
line 7
line 8
";
    let screen = run_test_screen(TestCase {
        screen_width: 10,
        screen_height: 4,
        content,
        events: vec![key('G'), key('g'), key('q')],
        ..Default::default()
    });
    let want = "\
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
";
    assert_eq!(screen.out(), want);
}

#[test]
fn cannot_past_boundaries() {
    let content = "\
line 1
line 2
line 3
";
    let screen = run_test_screen(TestCase {
        screen_width: 10,
        screen_height: 4,
        content,
        events: vec![key('k'), key('j'), key('q')],
        ..Default::default()
    });
    let want = "\
line 1
line 2
line 3
:
-----
[EVENT]:char:k
[EVENT]:char:j
[EVENT]:char:q
";
    assert_eq!(screen.out(), want);
}
