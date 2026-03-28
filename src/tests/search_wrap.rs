use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use pretty_assertions::assert_eq;

use super::{TestCase, key, run_test_screen};

fn enter() -> Event {
    Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
}

// Search match spans across the wrap boundary.
// The line "01234abcde" wraps at width 6 into "01234a" and "bcde".
// The match "ab" starts at the end of the first row and continues in the second.
#[test]
fn match_spanning_wrap_boundary() {
    let content = "\
line 1
01234abcde
line 3
";
    let screen = run_test_screen(TestCase {
        screen_width: 6,
        screen_height: 4,
        content,
        events: vec![key('/'), key('a'), key('b'), enter()],
        ..Default::default()
    });
    let want = "\
01234{reverse}a>
b{/reverse}cde
line 3
:
";
    assert_eq!(screen.last_snapshot(), want);
}

// Search match in the second row of a wrapped line (after the wrap boundary).
#[test]
fn match_in_second_row_of_wrapped_line() {
    let content = "\
line 1
01234abcde
line 3
";
    let screen = run_test_screen(TestCase {
        screen_width: 6,
        screen_height: 4,
        content,
        events: vec![key('/'), key('c'), key('d'), key('e'), enter()],
        ..Default::default()
    });
    let want = "\
01234a>
b{reverse}cde{/reverse}
line 3
:
";
    assert_eq!(screen.last_snapshot(), want);
}

// Search match in the first row of a wrapped line.
// The line "abcde_XX_fghij" wraps at width 10 into "abcde_XX_f" and "ghij".
// The match "XX" is only in the first row; the second row must render without error.
#[test]
fn match_in_first_row_of_wrapped_line() {
    let content = "\
abcde_XX_fghij
";
    let screen = run_test_screen(TestCase {
        screen_width: 10,
        screen_height: 4,
        content,
        events: vec![key('/'), key('X'), key('X'), enter()],
        ..Default::default()
    });
    let want = "\
abcde_{reverse}XX{/reverse}_f>
ghij
:

";
    assert_eq!(screen.last_snapshot(), want);
}
