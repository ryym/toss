use pretty_assertions::assert_eq;

use super::{TestCase, key, run_test_screen};

// "abcdefgh" wraps to "abcde" + "fgh" at width 5.
// Initial display shows them with soft wrap marker '>'.
#[test]
fn soft_wrap() {
    let out = run_test_screen(TestCase {
        screen_width: 5,
        screen_height: 4,
        content: "\
short
abcdefgh
end",
        events: vec![key('j'), key('j'), key('k'), key('q')],
    })
    .out();
    assert_eq!(
        out,
        "\
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
"
    );
}

// When scrolling down reveals a new wrap row, the entire visible
// portion of that line is redrawn to maintain soft wraps.
// Line "aaabbbccc" wraps to 3 rows at width 3.
#[test]
fn down_reveals_wrap_continuation() {
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

// Scrolling up to reveal a new wrap row that has the same line as rows below.
#[test]
fn up_reveals_wrap_start() {
    let out = run_test_screen(TestCase {
        screen_width: 5,
        screen_height: 4,
        content: "\
xx
abcdefgh
yy",
        events: vec![key('j'), key('j'), key('k'), key('q')],
    })
    .out();
    // "abcdefgh" wraps to "abcde" + "fgh"
    // Initial: xx, abcde>, fgh
    // j: abcde>, fgh, yy
    // j: can't scroll (yy is last)
    // k from [abcde>, fgh, yy]: back to [xx, abcde>, fgh]
    assert_eq!(
        out,
        "\
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
"
    );
}

#[test]
fn top_bottom_with_wrap() {
    let out = run_test_screen(TestCase {
        screen_width: 5,
        screen_height: 4,
        content: "\
line1
line2
abcdefgh",
        events: vec![key('G'), key('g'), key('q')],
    })
    .out();
    // "abcdefgh" wraps to "abcde" + "fgh"
    // Initial: line1, line2, abcde (fgh off-screen, no soft wrap)
    // G: line2, abcde>, fgh
    // g: line1, line2, abcde (fgh off-screen again)
    assert_eq!(
        out,
        "\
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
abcde
:
-----
[EVENT]:char:q
"
    );
}

// When the top of the screen shows the middle of a wrapped line,
// visible wrap rows should still be soft-wrapped together.
// "abcdefghijk" wraps to "abcde" + "fghij" + "k" at width 5.
#[test]
fn midline_at_top_of_screen() {
    let out = run_test_screen(TestCase {
        screen_width: 5,
        screen_height: 4,
        content: "\
abcdefghijk
end",
        events: vec![key('j'), key('q')],
    })
    .out();
    // Initial: abcde>, fghij>, k
    // j: fghij>, k, end
    // Even though "abcde" is off-screen, "fghij" and "k" should be
    // soft-wrapped together.
    assert_eq!(
        out,
        "\
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
"
    );
}
