use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use pretty_assertions::assert_eq;

use super::{TestCase, key, run_test_screen};

fn enter() -> Event {
    Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
}

// While the next match is already on screen, n moves the cursor without
// scrolling. Only once the next match falls off the bottom does the page
// scroll, and then only by the minimum needed to land it on the bottom edge.
#[test]
fn stays_then_min_scrolls_forward() {
    let content = "\
foo 0
foo 1
x
foo 2
foo 3
x
foo 4
";
    let screen = run_test_screen(TestCase {
        screen_width: 20,
        screen_height: 5,
        content,
        events: vec![
            key('/'),
            key('f'),
            key('o'),
            key('o'),
            enter(),  // foo 0 (top)
            key('n'), // foo 1: visible, no scroll
            key('n'), // foo 2: visible, no scroll
            key('n'), // foo 3: off the bottom, scroll just enough
        ],
        ..Default::default()
    });
    let want = "\
foo 0
foo 1
x
foo 2
:
-----
[EVENT]:char:/
foo 0
foo 1
x
foo 2
/█
-----
[EVENT]:char:f
{rev}{b}f{/rev}{/b}oo 0
{rev}{line}{b}f{/rev}{/line}{/b}oo 1
x
{rev}{line}{b}f{/rev}{/line}{/b}oo 2
/f█
-----
[EVENT]:char:o
{rev}{b}fo{/rev}{/b}o 0
{rev}{line}{b}f{/rev}{/line}{/b}{line}{b}o{/line}{/b}o 1
x
{rev}{line}{b}f{/rev}{/line}{/b}{line}{b}o{/line}{/b}o 2
/fo█
-----
[EVENT]:char:o
{rev}{b}foo{/rev}{/b} 0
{rev}{line}{b}f{/rev}{/line}{/b}{line}{b}oo{/line}{/b} 1
x
{rev}{line}{b}f{/rev}{/line}{/b}{line}{b}oo{/line}{/b} 2
/foo█
-----
[EVENT]:enter
{rev}{b}foo{/rev}{/b} 0
{rev}{line}{b}f{/rev}{/line}{/b}{line}{b}oo{/line}{/b} 1
x
{rev}{line}{b}f{/rev}{/line}{/b}{line}{b}oo{/line}{/b} 2
:
-----
[EVENT]:char:n
{rev}{line}{b}f{/rev}{/line}{/b}{line}{b}oo{/line}{/b} 0
{rev}{b}foo{/rev}{/b} 1
x
{rev}{line}{b}f{/rev}{/line}{/b}{line}{b}oo{/line}{/b} 2
:
-----
[EVENT]:char:n
{rev}{line}{b}f{/rev}{/line}{/b}{line}{b}oo{/line}{/b} 0
{rev}{line}{b}f{/rev}{/line}{/b}{line}{b}oo{/line}{/b} 1
x
{rev}{b}foo{/rev}{/b} 2
:
-----
[EVENT]:char:n
{rev}{line}{b}f{/rev}{/line}{/b}{line}{b}oo{/line}{/b} 1
x
{rev}{line}{b}f{/rev}{/line}{/b}{line}{b}oo{/line}{/b} 2
{rev}{b}foo{/rev}{/b} 3
:
-----
";
    assert_eq!(screen.out(), want);
}

// N is the upward mirror: it moves the cursor while the previous match is
// visible, then scrolls the minimum to land it on the top edge.
#[test]
fn stays_then_min_scrolls_backward() {
    let content = "\
foo 0
x
foo 1
foo 2
x
foo 3
foo 4
";
    let screen = run_test_screen(TestCase {
        screen_width: 20,
        screen_height: 5,
        content,
        events: vec![
            key('/'),
            key('f'),
            key('o'),
            key('o'),
            enter(),  // foo 0 (top)
            key('G'), // jump to end so foo 3 / foo 4 are visible
            key('N'), // re-anchor to first visible match (foo 3)
            key('N'), // foo 2: off the top, scroll just enough
        ],
        ..Default::default()
    });
    let want = "\
foo 0
x
foo 1
foo 2
:
-----
[EVENT]:char:/
foo 0
x
foo 1
foo 2
/█
-----
[EVENT]:char:f
{rev}{b}f{/rev}{/b}oo 0
x
{rev}{line}{b}f{/rev}{/line}{/b}oo 1
{rev}{line}{b}f{/rev}{/line}{/b}oo 2
/f█
-----
[EVENT]:char:o
{rev}{b}fo{/rev}{/b}o 0
x
{rev}{line}{b}f{/rev}{/line}{/b}{line}{b}o{/line}{/b}o 1
{rev}{line}{b}f{/rev}{/line}{/b}{line}{b}o{/line}{/b}o 2
/fo█
-----
[EVENT]:char:o
{rev}{b}foo{/rev}{/b} 0
x
{rev}{line}{b}f{/rev}{/line}{/b}{line}{b}oo{/line}{/b} 1
{rev}{line}{b}f{/rev}{/line}{/b}{line}{b}oo{/line}{/b} 2
/foo█
-----
[EVENT]:enter
{rev}{b}foo{/rev}{/b} 0
x
{rev}{line}{b}f{/rev}{/line}{/b}{line}{b}oo{/line}{/b} 1
{rev}{line}{b}f{/rev}{/line}{/b}{line}{b}oo{/line}{/b} 2
:
-----
[EVENT]:char:G
{rev}{line}{b}f{/rev}{/line}{/b}{line}{b}oo{/line}{/b} 2
x
{rev}{line}{b}f{/rev}{/line}{/b}{line}{b}oo{/line}{/b} 3
{rev}{line}{b}f{/rev}{/line}{/b}{line}{b}oo{/line}{/b} 4
:
-----
[EVENT]:char:N
{rev}{b}foo{/rev}{/b} 2
x
{rev}{line}{b}f{/rev}{/line}{/b}{line}{b}oo{/line}{/b} 3
{rev}{line}{b}f{/rev}{/line}{/b}{line}{b}oo{/line}{/b} 4
:
-----
[EVENT]:char:N
{rev}{b}foo{/rev}{/b} 1
{rev}{line}{b}f{/rev}{/line}{/b}{line}{b}oo{/line}{/b} 2
x
{rev}{line}{b}f{/rev}{/line}{/b}{line}{b}oo{/line}{/b} 3
:
-----
";
    assert_eq!(screen.out(), want);
}

// When the match sits on a later wrap row of a long line, that wrap row
// (not the line head) lands on the bottom edge.
#[test]
fn wrapped_match_lands_on_bottom_edge() {
    let content = "\
KEY here
filler1
filler2
filler3
abcdeKEYfg
";
    let screen = run_test_screen(TestCase {
        screen_width: 5,
        screen_height: 5,
        content,
        events: vec![
            key('/'),
            key('K'),
            key('E'),
            key('Y'),
            enter(),  // KEY on line 0 (top)
            key('n'), // KEY on wrap row 1 of the last line, lands at the bottom
        ],
        ..Default::default()
    });
    // The match's own wrap row 0 ("abcde") sits just above; only wrap row 1
    // ("KEYfg") reaches the bottom edge, confirming the anchor is the wrap row.
    let want = "\
KEY h>
ere
fille>
r1
:
-----
[EVENT]:char:/
KEY h>
ere
fille>
r1
/█
-----
[EVENT]:char:K
{rev}{b}K{/rev}{/b}EY h>
ere
fille>
r1
/K█
-----
[EVENT]:char:E
{rev}{b}KE{/rev}{/b}Y h>
ere
fille>
r1
/KE█
-----
[EVENT]:char:Y
{rev}{b}KEY{/rev}{/b} h>
ere
fille>
r1
/KEY█
-----
[EVENT]:enter
{rev}{b}KEY{/rev}{/b} h>
ere
fille>
r1
:
-----
[EVENT]:char:n
fille>
r3
abcde{rev}{b}>
KEY{/rev}{/b}fg
:
-----
";
    assert_eq!(screen.out(), want);
}
