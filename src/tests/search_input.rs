use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use pretty_assertions::assert_eq;

use super::{TestCase, backspace, ctrl, enter, esc, key, run_test};

fn left() -> Event {
    Event::Key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE))
}

fn right() -> Event {
    Event::Key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE))
}

// Typing / enters search mode and shows "/" on the status line.
// Typing characters triggers incremental search.
// Esc cancels and restores the original position.
#[test]
fn forward_search_input_and_cancel() {
    let content = "\
line 1
line 2
line 3
line 4
";
    let result = run_test(TestCase {
        screen_width: 10,
        screen_height: 4,
        content,
        events: vec![key('/'), key('a'), key('b'), esc(), key('q')],
        ..Default::default()
    });
    // No match for "a" or "ab" so position stays the same.
    // Esc restores original position.
    let want = "\
line 1
line 2
line 3
{rev} 1-3/4 75%{/rev}
-----
[EVENT]:char:/
line 1
line 2
line 3
/{rev} {/rev}
-----
[EVENT]:char:a
line 1
line 2
line 3
/a{rev} {/rev}
-----
[EVENT]:char:b
line 1
line 2
line 3
/ab{rev} {/rev}
-----
[EVENT]:esc
line 1
line 2
line 3
{rev} 1-3/4 75%{/rev}
-----
[EVENT]:char:q
";
    assert_eq!(result.output(), want);
}

// Typing ? enters backward search mode with "?" prompt.
// Enter submits and returns to view mode.
#[test]
fn backward_search_input_and_submit() {
    let content = "\
line 1
line 2
line 3
line 4
";
    let result = run_test(TestCase {
        screen_width: 10,
        screen_height: 4,
        content,
        events: vec![key('?'), key('x'), enter(), key('q')],
        ..Default::default()
    });
    let want = "\
line 1
line 2
line 3
{rev} 1-3/4 75%{/rev}
-----
[EVENT]:char:?
line 1
line 2
line 3
?{rev} {/rev}
-----
[EVENT]:char:x
line 1
line 2
line 3
?x{rev} {/rev}
-----
[EVENT]:enter
line 1
line 2
line 3
{rev} 1-3/4 75%{/rev}
-----
[EVENT]:char:q
";
    assert_eq!(result.output(), want);
}

// Backspace removes the last character from input.
#[test]
fn backspace_removes_character() {
    let content = "\
line 1
line 2
line 3
line 4
";
    let result = run_test(TestCase {
        screen_width: 10,
        screen_height: 4,
        content,
        events: vec![key('/'), key('a'), key('b'), backspace(), esc(), key('q')],
        ..Default::default()
    });
    let want = "\
line 1
line 2
line 3
{rev} 1-3/4 75%{/rev}
-----
[EVENT]:char:/
line 1
line 2
line 3
/{rev} {/rev}
-----
[EVENT]:char:a
line 1
line 2
line 3
/a{rev} {/rev}
-----
[EVENT]:char:b
line 1
line 2
line 3
/ab{rev} {/rev}
-----
[EVENT]:backspace
line 1
line 2
line 3
/a{rev} {/rev}
-----
[EVENT]:esc
line 1
line 2
line 3
{rev} 1-3/4 75%{/rev}
-----
[EVENT]:char:q
";
    assert_eq!(result.output(), want);
}

// Arrow keys move the cursor within the search input.
#[test]
fn arrow_keys_move_cursor() {
    let content = "\
line 1
line 2
line 3
line 4
";
    let result = run_test(TestCase {
        screen_width: 10,
        screen_height: 4,
        content,
        events: vec![
            key('/'),
            key('a'),
            key('b'),
            key('c'),
            left(),
            left(),
            key('x'),
            right(),
            esc(),
            key('q'),
        ],
        ..Default::default()
    });
    let want = "\
line 1
line 2
line 3
{rev} 1-3/4 75%{/rev}
-----
[EVENT]:char:/
line 1
line 2
line 3
/{rev} {/rev}
-----
[EVENT]:char:a
line 1
line 2
line 3
/a{rev} {/rev}
-----
[EVENT]:char:b
line 1
line 2
line 3
/ab{rev} {/rev}
-----
[EVENT]:char:c
line 1
line 2
line 3
/abc{rev} {/rev}
-----
[EVENT]:left
line 1
line 2
line 3
/ab{rev}c{/rev}
-----
[EVENT]:left
line 1
line 2
line 3
/a{rev}b{/rev}c
-----
[EVENT]:char:x
line 1
line 2
line 3
/ax{rev}b{/rev}c
-----
[EVENT]:right
line 1
line 2
line 3
/axb{rev}c{/rev}
-----
[EVENT]:esc
line 1
line 2
line 3
{rev} 1-3/4 75%{/rev}
-----
[EVENT]:char:q
";
    assert_eq!(result.output(), want);
}

// Ctrl-b / Ctrl-f move the cursor by one character, and Ctrl-a / Ctrl-e move it to
// either end of the input.
#[test]
fn ctrl_keys_move_cursor() {
    let content = "\
line 1
line 2
line 3
line 4
";
    let result = run_test(TestCase {
        screen_width: 10,
        screen_height: 4,
        content,
        events: vec![
            key('/'),
            key('a'),
            key('b'),
            ctrl('b'),
            key('x'),
            ctrl('f'),
            ctrl('a'),
            key('y'),
            ctrl('e'),
            key('z'),
            esc(),
            key('q'),
        ],
        ..Default::default()
    });
    let want = "\
line 1
line 2
line 3
{rev} 1-3/4 75%{/rev}
-----
[EVENT]:char:/
line 1
line 2
line 3
/{rev} {/rev}
-----
[EVENT]:char:a
line 1
line 2
line 3
/a{rev} {/rev}
-----
[EVENT]:char:b
line 1
line 2
line 3
/ab{rev} {/rev}
-----
[EVENT]:ctrl+char:b
line 1
line 2
line 3
/a{rev}b{/rev}
-----
[EVENT]:char:x
line 1
line 2
line 3
/ax{rev}b{/rev}
-----
[EVENT]:ctrl+char:f
line 1
line 2
line 3
/axb{rev} {/rev}
-----
[EVENT]:ctrl+char:a
line 1
line 2
line 3
/{rev}a{/rev}xb
-----
[EVENT]:char:y
line 1
line 2
line 3
/y{rev}a{/rev}xb
-----
[EVENT]:ctrl+char:e
line 1
line 2
line 3
/yaxb{rev} {/rev}
-----
[EVENT]:char:z
line 1
line 2
line 3
/yaxbz{rev} {/rev}
-----
[EVENT]:esc
line 1
line 2
line 3
{rev} 1-3/4 75%{/rev}
-----
[EVENT]:char:q
";
    assert_eq!(result.output(), want);
}

// Ctrl-k deletes from the cursor to the end, and the preview follows the shortened query.
#[test]
fn ctrl_k_deletes_to_end() {
    let content = "\
line 1
line 2
foo bar
line 4
line 5
line 6
";
    let result = run_test(TestCase {
        screen_width: 20,
        screen_height: 4,
        content,
        events: vec![
            key('/'),
            key('f'),
            key('o'),
            key('o'),
            ctrl('b'),
            ctrl('b'),
            ctrl('k'),
            esc(),
            key('q'),
        ],
        ..Default::default()
    });
    let want = "\
line 1
line 2
foo bar
{rev}lines 1-3/6 50%{/rev}
-----
[EVENT]:char:/
line 1
line 2
foo bar
/{rev} {/rev}
-----
[EVENT]:char:f
{rev}{b}f{/rev}{/b}oo bar
line 4
line 5
/f{rev} {/rev}
-----
[EVENT]:char:o
{rev}{b}fo{/rev}{/b}o bar
line 4
line 5
/fo{rev} {/rev}
-----
[EVENT]:char:o
{rev}{b}foo{/rev}{/b} bar
line 4
line 5
/foo{rev} {/rev}
-----
[EVENT]:ctrl+char:b
{rev}{b}foo{/rev}{/b} bar
line 4
line 5
/fo{rev}o{/rev}
-----
[EVENT]:ctrl+char:b
{rev}{b}foo{/rev}{/b} bar
line 4
line 5
/f{rev}o{/rev}o
-----
[EVENT]:ctrl+char:k
{rev}{b}f{/rev}{/b}oo bar
line 4
line 5
/f{rev} {/rev}
-----
[EVENT]:esc
line 1
line 2
foo bar
{rev}lines 1-3/6 50%{/rev}
-----
[EVENT]:char:q
";
    assert_eq!(result.output(), want);
}

// Ctrl-h behaves like Backspace, including leaving search input once it is empty.
#[test]
fn ctrl_h_deletes_character_and_cancels_when_empty() {
    let content = "\
line 1
line 2
line 3
line 4
";
    let result = run_test(TestCase {
        screen_width: 10,
        screen_height: 4,
        content,
        events: vec![key('/'), key('a'), ctrl('h'), ctrl('h'), key('q')],
        ..Default::default()
    });
    let want = "\
line 1
line 2
line 3
{rev} 1-3/4 75%{/rev}
-----
[EVENT]:char:/
line 1
line 2
line 3
/{rev} {/rev}
-----
[EVENT]:char:a
line 1
line 2
line 3
/a{rev} {/rev}
-----
[EVENT]:ctrl+char:h
line 1
line 2
line 3
/{rev} {/rev}
-----
[EVENT]:ctrl+char:h
line 1
line 2
line 3
{rev} 1-3/4 75%{/rev}
-----
[EVENT]:char:q
";
    assert_eq!(result.output(), want);
}

// Ctrl-g and Ctrl-c both cancel search input like Esc. Ctrl-c quits in view mode, but
// here it must not: the following `j` still scrolls.
#[test]
fn ctrl_g_and_ctrl_c_cancel_search_input() {
    let content = "\
line 1
line 2
line 3
line 4
";
    let result = run_test(TestCase {
        screen_width: 10,
        screen_height: 4,
        content,
        events: vec![
            key('/'),
            key('a'),
            ctrl('g'),
            key('/'),
            key('b'),
            ctrl('c'),
            key('j'),
            key('q'),
        ],
        ..Default::default()
    });
    let want = "\
line 1
line 2
line 3
{rev} 1-3/4 75%{/rev}
-----
[EVENT]:char:/
line 1
line 2
line 3
/{rev} {/rev}
-----
[EVENT]:char:a
line 1
line 2
line 3
/a{rev} {/rev}
-----
[EVENT]:ctrl+char:g
line 1
line 2
line 3
{rev} 1-3/4 75%{/rev}
-----
[EVENT]:char:/
line 1
line 2
line 3
/{rev} {/rev}
-----
[EVENT]:char:b
line 1
line 2
line 3
/b{rev} {/rev}
-----
[EVENT]:ctrl+char:c
line 1
line 2
line 3
{rev} 1-3/4 75%{/rev}
-----
[EVENT]:char:j
line 2
line 3
line 4
{rev}2-4/4 100%{/rev}
-----
[EVENT]:char:q
";
    assert_eq!(result.output(), want);
}

// An unbound Ctrl key does nothing rather than inserting its letter.
#[test]
fn unbound_ctrl_key_does_not_insert_character() {
    let content = "\
line 1
line 2
line 3
line 4
";
    let result = run_test(TestCase {
        screen_width: 10,
        screen_height: 4,
        content,
        events: vec![key('/'), key('a'), ctrl('z'), key('b'), esc(), key('q')],
        ..Default::default()
    });
    // Ctrl-z changes nothing, so no snapshot follows its event.
    let want = "\
line 1
line 2
line 3
{rev} 1-3/4 75%{/rev}
-----
[EVENT]:char:/
line 1
line 2
line 3
/{rev} {/rev}
-----
[EVENT]:char:a
line 1
line 2
line 3
/a{rev} {/rev}
-----
[EVENT]:ctrl+char:z
[EVENT]:char:b
line 1
line 2
line 3
/ab{rev} {/rev}
-----
[EVENT]:esc
line 1
line 2
line 3
{rev} 1-3/4 75%{/rev}
-----
[EVENT]:char:q
";
    assert_eq!(result.output(), want);
}
