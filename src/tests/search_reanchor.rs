use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use pretty_assertions::assert_eq;

use super::{TestCase, key, run_test_screen};
use crate::options::{HeadingOptions, Options};

fn enter() -> Event {
    Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
}

fn heading_opts(pattern: &str, num_lines: usize) -> Option<HeadingOptions> {
    Some(HeadingOptions {
        pattern: regex::Regex::new(pattern).unwrap(),
        num_lines,
    })
}

// When the cursor is outside the viewport and visible matches exist,
// pressing n re-anchors the cursor to the first visible match without scrolling.
// The second n then jumps forward from the re-anchored position.
#[test]
fn reanchor_to_first_visible_match() {
    let content = "\
A 1
line 2
line 3
line 4
line 5
line 6
A 7
line 8
A 9
line 10
";
    let screen = run_test_screen(TestCase {
        screen_width: 20,
        screen_height: 4,
        content,
        events: vec![
            key('/'),
            key('A'),
            enter(),
            key('G'), // Jump to end — cursor ("A 1") goes off-screen.
            key('n'), // Re-anchors to "A 9" (first visible match). No scroll.
            key('n'), // Jumps from "A 9" forward, wrapping to "A 1".
        ],
        ..Default::default()
    });
    let want = "\
A 1
line 2
line 3
:
-----
[EVENT]:char:/
A 1
line 2
line 3
/█
-----
[EVENT]:char:A
{reverse}A{/reverse} 1
line 2
line 3
/A█
-----
[EVENT]:enter
{reverse}A{/reverse} 1
line 2
line 3
:
-----
[EVENT]:char:G
line 8
{dim}{reverse}A{/reverse}{/dim} 9
line 10
:
-----
[EVENT]:char:n
line 8
{reverse}A{/reverse} 9
line 10
:
-----
[EVENT]:char:n
{reverse}A{/reverse} 1
line 2
line 3
:
-----
";
    assert_eq!(screen.out(), want);
}

// When the cursor is outside the viewport and no matches are visible,
// pressing n searches forward from the viewport's top line.
#[test]
fn reanchor_searches_from_viewport_top() {
    let content = "\
A 1
line 2
line 3
line 4
line 5
line 6
line 7
A 8
";
    let screen = run_test_screen(TestCase {
        screen_width: 20,
        screen_height: 4,
        content,
        events: vec![
            key('/'),
            key('A'),
            enter(),
            // Scroll down so cursor ("A 1") is off-screen and no matches visible.
            key('j'),
            key('j'),
            key('j'),
            key('n'), // Searches forward from viewport top, finds "A 8".
        ],
        ..Default::default()
    });
    let want = "\
A 1
line 2
line 3
:
-----
[EVENT]:char:/
A 1
line 2
line 3
/█
-----
[EVENT]:char:A
{reverse}A{/reverse} 1
line 2
line 3
/A█
-----
[EVENT]:enter
{reverse}A{/reverse} 1
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
[EVENT]:char:j
line 4
line 5
line 6
:
-----
[EVENT]:char:n
line 6
line 7
{reverse}A{/reverse} 8
:
-----
";
    assert_eq!(screen.out(), want);
}

// When the cursor is still within the viewport, n jumps normally.
#[test]
fn no_reanchor_when_cursor_visible() {
    let content = "\
line 1
A 2
line 3
A 4
line 5
line 6
";
    let screen = run_test_screen(TestCase {
        screen_width: 20,
        screen_height: 4,
        content,
        events: vec![
            key('/'),
            key('A'),
            enter(),
            key('k'), // Scroll up 1. Cursor ("A 2") is still visible.
            key('n'), // Cursor visible, so normal jump to "A 4".
        ],
        ..Default::default()
    });
    let want = "\
line 1
A 2
line 3
:
-----
[EVENT]:char:/
line 1
A 2
line 3
/█
-----
[EVENT]:char:A
{reverse}A{/reverse} 2
line 3
{dim}{reverse}A{/reverse}{/dim} 4
/A█
-----
[EVENT]:enter
{reverse}A{/reverse} 2
line 3
{dim}{reverse}A{/reverse}{/dim} 4
:
-----
[EVENT]:char:k
line 1
{reverse}A{/reverse} 2
line 3
:
-----
[EVENT]:char:n
{reverse}A{/reverse} 4
line 5
line 6
:
-----
";
    assert_eq!(screen.out(), want);
}

// Reverse search (N) also re-anchors when cursor is off-screen.
#[test]
fn reanchor_reverse_direction() {
    let content = "\
line 1
line 2
line 3
A 4
line 5
A 6
line 7
A 8
";
    let screen = run_test_screen(TestCase {
        screen_width: 20,
        screen_height: 4,
        content,
        events: vec![
            key('/'),
            key('A'),
            enter(),
            key('G'), // Jump to end — cursor ("A 4") goes off-screen.
            key('N'), // Re-anchors to first visible match.
            key('N'), // Jumps backward from the re-anchored position.
        ],
        ..Default::default()
    });
    // Re-anchored to "A 6" (first visible), then N backward to "A 4".
    let want = "\
line 1
line 2
line 3
:
-----
[EVENT]:char:/
line 1
line 2
line 3
/█
-----
[EVENT]:char:A
{reverse}A{/reverse} 4
line 5
{dim}{reverse}A{/reverse}{/dim} 6
/A█
-----
[EVENT]:enter
{reverse}A{/reverse} 4
line 5
{dim}{reverse}A{/reverse}{/dim} 6
:
-----
[EVENT]:char:G
{dim}{reverse}A{/reverse}{/dim} 6
line 7
{dim}{reverse}A{/reverse}{/dim} 8
:
-----
[EVENT]:char:N
{reverse}A{/reverse} 6
line 7
{dim}{reverse}A{/reverse}{/dim} 8
:
-----
[EVENT]:char:N
{reverse}A{/reverse} 4
line 5
{dim}{reverse}A{/reverse}{/dim} 6
:
-----
";
    assert_eq!(screen.out(), want);
}

// When a heading overlay hides a match, re-anchor should skip it
// and find the first truly visible match (not one behind the overlay).
#[test]
fn reanchor_skips_match_hidden_by_heading() {
    // Section header "# Sec" is sticky (1 line).
    // After scrolling, the overlay hides the first viewport row.
    let content = "\
# Sec
A 1
line 2
line 3
line 4
A 5
line 6
A 7
line 8
";
    let screen = run_test_screen(TestCase {
        screen_width: 20,
        screen_height: 5,
        content,
        options: Options {
            heading: heading_opts("^# ", 1),
            ..Default::default()
        },
        events: vec![
            key('/'),
            key('A'),
            enter(),
            // Jump to end — cursor ("A 1") goes off-screen.
            // The sticky header "# Sec" overlays the first viewport row.
            key('G'),
            // n: re-anchor. "A 5" is in the viewport rows but hidden by overlay.
            // Should re-anchor to "A 7" (first actually visible match).
            key('n'),
            // n: jump from "A 7" forward, wrapping to "A 1".
            key('n'),
        ],
        ..Default::default()
    });
    // Final state shows "A 1" — proves re-anchor landed on "A 7", not "A 5".
    let want = "\
# Sec
A 1
line 2
line 3
:
-----
[EVENT]:char:/
# Sec
A 1
line 2
line 3
/█
-----
[EVENT]:char:A
# Sec
{reverse}A{/reverse} 1
line 2
line 3
/A█
-----
[EVENT]:enter
# Sec
{reverse}A{/reverse} 1
line 2
line 3
:
-----
[EVENT]:char:G
# Sec
line 6
{dim}{reverse}A{/reverse}{/dim} 7
line 8
:
-----
[EVENT]:char:n
# Sec
line 6
{reverse}A{/reverse} 7
line 8
:
-----
[EVENT]:char:n
# Sec
{reverse}A{/reverse} 1
line 2
line 3
:
-----
";
    assert_eq!(screen.out(), want);
}

// When a long line wraps and only the later wrap rows are visible,
// a match on an off-screen wrap row should not be treated as visible.
// Re-anchor should find the first match on a visible wrap row.
#[test]
fn reanchor_with_wrapped_line() {
    // At width 5, line 1 "foo12foo34end" wraps to:
    //   wrap row 0: "foo12" (match index 0)
    //   wrap row 1: "foo34" (match index 1)
    //   wrap row 2: "end"
    // After scrolling, wrap row 0 goes off-screen.
    // "last foo" wraps to "last " + "foo", so the match is on wrap row 1.
    let content = "\
short
foo12foo34end
last-foo
";
    let screen = run_test_screen(TestCase {
        screen_width: 5,
        screen_height: 4,
        content,
        events: vec![
            key('/'),
            key('f'),
            key('o'),
            key('o'),
            enter(),
            // Scroll so wrap row 0 of line 1 (with match 0) goes off-screen.
            key('j'),
            key('j'),
            // n: cursor (match 0, wrap row 0) is off-screen.
            // Should re-anchor to the first match on a visible wrap row.
            // Wrap row 2 of line 1 ("end") has no match.
            // Line 2 wrap row 1 ("foo") has the match → re-anchor there.
            key('n'),
        ],
        ..Default::default()
    });
    let want = "\
short
foo12>
foo34
:
-----
[EVENT]:char:/
short
foo12>
foo34
/█
-----
[EVENT]:char:f
{reverse}f{/reverse}oo12{dim}{reverse}>
f{/reverse}{/dim}oo34>
end
/f█
-----
[EVENT]:char:o
{reverse}fo{/reverse}o12{dim}{reverse}>
fo{/reverse}{/dim}o34>
end
/fo█
-----
[EVENT]:char:o
{reverse}foo{/reverse}12{dim}{reverse}>
foo{/reverse}{/dim}34>
end
/foo█
-----
[EVENT]:enter
{reverse}foo{/reverse}12{dim}{reverse}>
foo{/reverse}{/dim}34>
end
:
-----
[EVENT]:char:j
foo{/reverse}{/dim}34>
end
last-
:
-----
[EVENT]:char:j
end
last-{dim}{reverse}>
foo{/reverse}{/dim}
:
-----
[EVENT]:char:n
end
last-{reverse}>
foo{/reverse}
:
-----
";
    assert_eq!(screen.out(), want);
}
