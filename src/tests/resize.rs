use pretty_assertions::assert_eq;

use super::{TestCase, key, resize, run_test_screen};

// Growing the screen after scrolling should keep the current top line anchored
// and fill the newly available rows below it with more content.
#[test]
fn grow_after_scroll() {
    let content = "\
line 1
line 2
line 3
line 4
line 5
line 6
line 7
line 8
line 9
line 10
";
    let screen = run_test_screen(TestCase {
        screen_width: 10,
        screen_height: 4,
        content,
        events: vec![key('j'), key('j'), resize(10, 6), key('q')],
        ..Default::default()
    });
    let want = "\
line 1
line 2
line 3
{rev}1-3/10 30%{/rev}
-----
[EVENT]:char:j
line 2
line 3
line 4
{rev}2-4/10 40%{/rev}
-----
[EVENT]:char:j
line 3
line 4
line 5
{rev}3-5/10 50%{/rev}
-----
[EVENT]:resize:10x6
line 3
line 4
line 5
line 6
line 7
{rev}3-7/10 70%{/rev}
-----
[EVENT]:char:q
";
    assert_eq!(screen.out(), want);
}

// Shrinking the screen after scrolling should keep the current top line anchored,
// shrink the visible content to fit, and leave no stale rows behind from the
// larger previous size.
#[test]
fn shrink_after_scroll() {
    let content = "\
line 1
line 2
line 3
line 4
line 5
line 6
line 7
line 8
line 9
line 10
";
    let screen = run_test_screen(TestCase {
        screen_width: 10,
        screen_height: 8,
        content,
        events: vec![key('j'), key('j'), resize(10, 4), key('q')],
        ..Default::default()
    });
    let want = "\
line 1
line 2
line 3
line 4
line 5
line 6
line 7
{rev}1-7/10 70%{/rev}
-----
[EVENT]:char:j
line 2
line 3
line 4
line 5
line 6
line 7
line 8
{rev}2-8/10 80%{/rev}
-----
[EVENT]:char:j
line 3
line 4
line 5
line 6
line 7
line 8
line 9
{rev}3-9/10 90%{/rev}
-----
[EVENT]:resize:10x4
line 3
line 4
line 5
{rev}3-5/10 50%{/rev}
-----
[EVENT]:char:q
";
    assert_eq!(screen.out(), want);
}

// Widening the screen after content has wrapped should reflow it without a soft wrap,
// and narrowing it back should reflow it with a soft wrap again. Also exercises growing
// the grid and shrinking it back, checking no stale rows leak from a previous size.
#[test]
fn width_change_reflows_wrapped_lines() {
    let content = "abcdefgh\nshort\n";
    let screen = run_test_screen(TestCase {
        screen_width: 5,
        screen_height: 5,
        content,
        events: vec![resize(10, 5), resize(5, 5), key('q')],
        ..Default::default()
    });
    let want = "\
abcde>
fgh
short
{rev} 100%{/rev}

-----
[EVENT]:resize:10x5
abcdefgh
short
{rev}1-2/2 100%{/rev}


-----
[EVENT]:resize:5x5
abcde>
fgh
short
{rev} 100%{/rev}

-----
[EVENT]:char:q
";
    assert_eq!(screen.out(), want);
}

// Scrolling up after growing the screen while at the end of the document should
// move the status line to the new bottom row and leave no stale copy behind.
#[test]
fn repro_grow_at_end_then_scroll_up() {
    let content = "\
line 1
line 2
line 3
line 4
line 5
line 6
line 7
line 8
line 9
line 10
";
    let screen = run_test_screen(TestCase {
        screen_width: 10,
        screen_height: 4,
        content,
        events: vec![
            key('G'),
            resize(10, 8),
            key('k'),
            key('k'),
            key('k'),
            key('q'),
        ],
        ..Default::default()
    });
    let want = "\
line 1
line 2
line 3
{rev}1-3/10 30%{/rev}
-----
[EVENT]:char:G
line 8
line 9
line 10
{rev}10/10 100%{/rev}
-----
[EVENT]:resize:10x8
line 8
line 9
line 10
{rev}10/10 100%{/rev}




-----
[EVENT]:char:k
line 7
line 8
line 9
{rev}7-9/10 90%{/rev}
{rev}10/10 100%{/rev}



-----
[EVENT]:char:k
line 6
line 7
line 8
{rev}6-8/10 80%{/rev}
{rev}7-9/10 90%{/rev}
{rev}10/10 100%{/rev}


-----
[EVENT]:char:k
line 5
line 6
line 7
{rev}5-7/10 70%{/rev}
{rev}6-8/10 80%{/rev}
{rev}7-9/10 90%{/rev}
{rev}10/10 100%{/rev}

-----
[EVENT]:char:q
";
    assert_eq!(screen.out(), want);
}

// Growing the screen while at the end of the document should re-anchor the top
// upward to fill the newly available rows, keeping the last line pinned at the
// bottom instead of leaving blank space below it. Further scrolling up from
// there is then ordinary scrolling.
#[test]
// BUG: See the `repro_grow_at_end_then_scroll_up` case.
#[should_panic]
fn resize_at_end_fills_screen_from_above() {
    let content = "\
line 1
line 2
line 3
line 4
line 5
line 6
line 7
line 8
line 9
line 10
";
    let screen = run_test_screen(TestCase {
        screen_width: 10,
        screen_height: 4,
        content,
        events: vec![
            key('G'),
            resize(10, 8),
            key('k'),
            key('k'),
            key('k'),
            key('q'),
        ],
        ..Default::default()
    });
    let want = "\
line 1
line 2
line 3
{rev}1-3/10 30%{/rev}
-----
[EVENT]:char:G
line 8
line 9
line 10
{rev}10/10 100%{/rev}
-----
[EVENT]:resize:10x8
line 4
line 5
line 6
line 7
line 8
line 9
line 10
{rev}4-10/10 100%{/rev}
-----
[EVENT]:char:k
line 3
line 4
line 5
line 6
line 7
line 8
line 9
{rev}3-9/10 90%{/rev}
-----
[EVENT]:char:k
line 2
line 3
line 4
line 5
line 6
line 7
line 8
{rev}2-8/10 80%{/rev}
-----
[EVENT]:char:k
line 1
line 2
line 3
line 4
line 5
line 6
line 7
{rev}1-7/10 70%{/rev}
-----
[EVENT]:char:q
";
    assert_eq!(screen.out(), want);
}

// Growing the screen past the document's total size should show the whole
// document, padded with blank rows below. From there, both directions are
// already fully shown, so scrolling either way does nothing.
#[test]
// BUG: See the `repro_grow_at_end_then_scroll_up` case.
#[should_panic]
fn resize_screen_larger_than_document_shows_whole_document() {
    let content = "\
line 1
line 2
line 3
line 4
line 5
line 6
line 7
line 8
line 9
line 10
";
    let screen = run_test_screen(TestCase {
        screen_width: 10,
        screen_height: 4,
        content,
        events: vec![key('G'), resize(10, 15), key('k'), key('j'), key('q')],
        ..Default::default()
    });
    let want = "\
line 1
line 2
line 3
{rev}1-3/10 30%{/rev}
-----
[EVENT]:char:G
line 8
line 9
line 10
{rev}10/10 100%{/rev}
-----
[EVENT]:resize:10x15
line 1
line 2
line 3
line 4
line 5
line 6
line 7
line 8
line 9
line 10
{rev}10/10 100%{/rev}




-----
[EVENT]:char:k
[EVENT]:char:j
[EVENT]:char:q
";
    assert_eq!(screen.out(), want);
}
