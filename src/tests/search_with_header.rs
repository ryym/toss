use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use pretty_assertions::assert_eq;

use super::{TestCase, key, run_test_screen};
use crate::options::Options;

fn enter() -> Event {
    Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
}

/// When searching with a global header,
/// the matched line is visible below the header, not hidden behind it.
#[test]
fn search_with_global_header() {
    let content = "\
# Title
line 1
line 2
line 3
line 4
line 5
";
    let screen = run_test_screen(TestCase {
        screen_width: 20,
        screen_height: 4,
        content,
        options: Options {
            header: 1,
            ..Default::default()
        },
        events: vec![key('/'), key('3'), enter()],
        ..Default::default()
    });
    let want = "\
# Title
line 1
line 2
:
-----
[EVENT]:char:/
# Title
line 1
line 2
/█
-----
[EVENT]:char:3
# Title
line {reverse}3{/reverse}
line 4
/3█
-----
[EVENT]:enter
# Title
line {reverse}3{/reverse}
line 4
:
-----
";
    assert_eq!(want, screen.out());
}

/// When searching and jumping with a global header,
/// the matched line is visible below the header, not hidden behind it.
#[test]
fn search_jump_with_global_header() {
    let content = "\
# Title
A
line 1
line 2
AB
line 4
line 5
AC
line 6
";
    let screen = run_test_screen(TestCase {
        screen_width: 20,
        screen_height: 5,
        content,
        options: Options {
            header: 1,
            ..Default::default()
        },
        events: vec![
            // Search by "A"
            key('/'),
            key('A'),
            enter(),
            // Jump around
            key('n'),
            key('n'),
            key('N'),
        ],
        ..Default::default()
    });
    let want = "\
# Title
A
line 1
line 2
:
-----
[EVENT]:char:/
# Title
A
line 1
line 2
/█
-----
[EVENT]:char:A
# Title
{reverse}A{/reverse}
line 1
line 2
/A█
-----
[EVENT]:enter
# Title
{reverse}A{/reverse}
line 1
line 2
:
-----
[EVENT]:char:n
# Title
{reverse}A{/reverse}B
line 4
line 5
:
-----
[EVENT]:char:n
# Title
line 5
{reverse}A{/reverse}C
line 6
:
-----
[EVENT]:char:N
# Title
{reverse}A{/reverse}B
line 4
line 5
:
-----
";
    assert_eq!(want, screen.out());
}
