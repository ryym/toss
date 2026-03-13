use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use pretty_assertions::assert_eq;

use super::{TestCase, key, run_test_screen};

fn enter() -> Event {
    Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
}

// Search match in the first row of a wrapped line.
// The line "abcde_XX_fghij" wraps at width 10 into "abcde_XX_f" and "ghij".
// The match "XX" is only in the first row; the second row must render without error.
#[test]
fn match_in_first_row_of_wrapped_line() {
    let screen = run_test_screen(TestCase {
        screen_width: 10,
        screen_height: 4,
        content: "abcde_XX_fghij",
        events: vec![key('/'), key('X'), key('X'), enter()],
    });
    assert_eq!(
        screen.last_snapshot(),
        "\
abcde_{reverse}XX{/reverse}_f>
ghij
:

"
    );
}
