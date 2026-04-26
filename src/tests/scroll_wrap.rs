use pretty_assertions::assert_eq;

use super::{TestCase, key, run_test_screen};

// "abcdefgh" wraps to "abcde" + "fgh" at width 5.
// Initial display shows them with soft wrap marker '>'.
#[test]
fn soft_wrap() {
    let content = "\
short
abcdefgh
end
";
    let screen = run_test_screen(TestCase {
        screen_width: 5,
        screen_height: 4,
        content,
        events: vec![key('j'), key('j'), key('k'), key('q')],
        ..Default::default()
    });
    let want = "\
short
abcde>
fgh
:
-----
[EVENT]:char:j
abcde>
fgh
end
:
-----
[EVENT]:char:j
[EVENT]:char:k
short
abcde>
fgh
:
-----
[EVENT]:char:q
";
    assert_eq!(screen.out(), want);
}

// When scrolling down reveals a new wrap row, the entire visible
// portion of that line is redrawn to maintain soft wraps.
// Line "aaabbbccc" wraps to 3 rows at width 3.
#[test]
fn down_reveals_wrap_continuation() {
    let content = "\
xx
aaabbbccc
yy
";
    let screen = run_test_screen(TestCase {
        screen_width: 3,
        screen_height: 5,
        content,
        events: vec![key('j'), key('j'), key('q')],
        ..Default::default()
    });
    // Initial: xx, aaa>, bbb>, ccc
    // After j: aaa>, bbb>, ccc, yy
    // After j: can't scroll (yy is last)
    let want = "\
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
";
    assert_eq!(screen.out(), want);
}

// Scrolling up to reveal a new wrap row that has the same line as rows below.
#[test]
fn up_reveals_wrap_start() {
    let content = "\
xx
abcdefgh
yy
";
    let screen = run_test_screen(TestCase {
        screen_width: 5,
        screen_height: 4,
        content,
        events: vec![key('j'), key('j'), key('k'), key('q')],
        ..Default::default()
    });
    // "abcdefgh" wraps to "abcde" + "fgh"
    // Initial: xx, abcde>, fgh
    // j: abcde>, fgh, yy
    // j: can't scroll (yy is last)
    // k from [abcde>, fgh, yy]: back to [xx, abcde>, fgh]
    let want = "\
xx
abcde>
fgh
:
-----
[EVENT]:char:j
abcde>
fgh
yy
:
-----
[EVENT]:char:j
[EVENT]:char:k
xx
abcde>
fgh
:
-----
[EVENT]:char:q
";
    assert_eq!(screen.out(), want);
}

#[test]
fn top_bottom_with_wrap() {
    let content = "\
line1
line2
abcdefgh
";
    let screen = run_test_screen(TestCase {
        screen_width: 5,
        screen_height: 4,
        content,
        events: vec![key('G'), key('g'), key('q')],
        ..Default::default()
    });
    // "abcdefgh" wraps to "abcde" + "fgh"
    // Initial: line1, line2, abcde (fgh off-screen, no soft wrap)
    // G: line2, abcde>, fgh
    // g: line1, line2, abcde (fgh off-screen again)
    let want = "\
line1
line2
abcde
:
-----
[EVENT]:char:G
line2
abcde>
fgh
:
-----
[EVENT]:char:g
line1
line2
abcde>
:
-----
[EVENT]:char:q
";
    assert_eq!(screen.out(), want);
}

// When the top of the screen shows the middle of a wrapped line,
// visible wrap rows should still be soft-wrapped together.
// "abcdefghijk" wraps to "abcde" + "fghij" + "k" at width 5.
#[test]
fn midline_at_top_of_screen() {
    let content = "\
abcdefghijk
end
";
    let screen = run_test_screen(TestCase {
        screen_width: 5,
        screen_height: 4,
        content,
        events: vec![key('j'), key('q')],
        ..Default::default()
    });
    // Initial: abcde>, fghij>, k
    // j: fghij>, k, end
    // Even though "abcde" is off-screen, "fghij" and "k" should be
    // soft-wrapped together.
    let want = "\
abcde>
fghij>
k
:
-----
[EVENT]:char:j
fghij>
k
end
:
-----
[EVENT]:char:q
";
    assert_eq!(screen.out(), want);
}
