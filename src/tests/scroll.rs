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
 1-3/5 60%
-----
[EVENT]:char:j
line 2
line 3
line 4
 2-4/5 80%
-----
[EVENT]:char:j
line 3
line 4
line 5
3-5/5 100%
-----
[EVENT]:char:k
line 2
line 3
line 4
 2-4/5 80%
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
 1-4/8 50%
-----
[EVENT]:char:d
line 3
line 4
line 5
line 6
 3-6/8 75%
-----
[EVENT]:char:u
line 1
line 2
line 3
line 4
 1-4/8 50%
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
 1-3/8 37%
-----
[EVENT]:char:G
line 6
line 7
line 8
6-8/8 100%
-----
[EVENT]:char:g
line 1
line 2
line 3
 1-3/8 37%
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
1-3/3 100%
-----
[EVENT]:char:k
[EVENT]:char:j
[EVENT]:char:q
";
    assert_eq!(screen.out(), want);
}
