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
        events: vec![key('/'), key('a'), key('b'), enter(), key('q')],
        ..Default::default()
    });
    let want = "\
line 1
01234a>
bcde
{rev}/3 66%{/rev}
-----
[EVENT]:char:/
line 1
01234a>
bcde
/█
-----
[EVENT]:char:a
01234{rev}{b}a{/rev}{/b}>
bcde
line 3
/a█
-----
[EVENT]:char:b
01234{rev}{b}a>
b{/rev}{/b}cde
line 3
/ab█
-----
[EVENT]:enter
01234{rev}{b}a>
b{/rev}{/b}cde
line 3
{rev}3 100%{/rev}
-----
[EVENT]:char:q
";
    assert_eq!(screen.out(), want);
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
        events: vec![key('/'), key('c'), key('d'), key('e'), enter(), key('q')],
        ..Default::default()
    });
    let want = "\
line 1
01234a>
bcde
{rev}/3 66%{/rev}
-----
[EVENT]:char:/
line 1
01234a>
bcde
/█
-----
[EVENT]:char:c
01234a>
b{rev}{b}c{/rev}{/b}de
line 3
/c█
-----
[EVENT]:char:d
01234a>
b{rev}{b}cd{/rev}{/b}e
line 3
/cd█
-----
[EVENT]:char:e
01234a>
b{rev}{b}cde{/rev}{/b}
line 3
/cde█
-----
[EVENT]:enter
01234a>
b{rev}{b}cde{/rev}{/b}
line 3
{rev}3 100%{/rev}
-----
[EVENT]:char:q
";
    assert_eq!(screen.out(), want);
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
        events: vec![key('/'), key('X'), key('X'), enter(), key('q')],
        ..Default::default()
    });
    let want = "\
abcde_XX_f>
ghij
{rev}1-1/1 100%{/rev}

-----
[EVENT]:char:/
abcde_XX_f>
ghij
/█

-----
[EVENT]:char:X
abcde_{rev}{b}X{/rev}{/b}{rev}{line}{b}X{/rev}{/line}{/b}_f>
ghij
/X█

-----
[EVENT]:char:X
abcde_{rev}{b}XX{/rev}{/b}_f>
ghij
/XX█

-----
[EVENT]:enter
abcde_{rev}{b}XX{/rev}{/b}_f>
ghij
{rev}1-1/1 100%{/rev}

-----
[EVENT]:char:q
";
    assert_eq!(screen.out(), want);
}
