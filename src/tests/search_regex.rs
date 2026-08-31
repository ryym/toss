use pretty_assertions::assert_eq;

use super::{TestCase, backspace, enter, esc, key, run_test_screen};

// Metacharacters in the input are interpreted as regex syntax, not literal text.
#[test]
fn metacharacters_are_interpreted_as_regex() {
    let content = "\
line 1
abc
line 3
";
    let screen = run_test_screen(TestCase {
        screen_width: 20,
        screen_height: 4,
        content,
        events: vec![key('/'), key('a'), key('.'), key('c'), esc(), key('q')],
        ..Default::default()
    });
    let want = "\
line 1
abc
line 3
{rev}lines 1-3/3 100%{/rev}
-----
[EVENT]:char:/
line 1
abc
line 3
/█
-----
[EVENT]:char:a
line 1
{rev}{b}a{/rev}{/b}bc
line 3
/a█
-----
[EVENT]:char:.
line 1
{rev}{b}ab{/rev}{/b}c
line 3
/a.█
-----
[EVENT]:char:c
line 1
{rev}{b}abc{/rev}{/b}
line 3
/a.c█
-----
[EVENT]:esc
line 1
abc
line 3
{rev}lines 1-3/3 100%{/rev}
-----
[EVENT]:char:q
";
    assert_eq!(screen.out(), want);
}

// While the raw input is a syntactically invalid regex (e.g. right after typing an
// unmatched `(`), the preview must freeze instead of panicking or clearing, and the
// input text still echoes live.
#[test]
fn invalid_regex_freezes_preview_until_valid() {
    let content = "\
line 1
foo bar
line 3
";
    let screen = run_test_screen(TestCase {
        screen_width: 20,
        screen_height: 4,
        content,
        events: vec![
            key('/'),
            key('('),
            key('f'),
            key('o'),
            key('o'),
            key(')'),
            esc(),
            key('q'),
        ],
        ..Default::default()
    });
    let want = "\
line 1
foo bar
line 3
{rev}lines 1-3/3 100%{/rev}
-----
[EVENT]:char:/
line 1
foo bar
line 3
/█
-----
[EVENT]:char:(
line 1
foo bar
line 3
/(█
-----
[EVENT]:char:f
line 1
foo bar
line 3
/(f█
-----
[EVENT]:char:o
line 1
foo bar
line 3
/(fo█
-----
[EVENT]:char:o
line 1
foo bar
line 3
/(foo█
-----
[EVENT]:char:)
line 1
{rev}{b}foo{/rev}{/b} bar
line 3
/(foo)█
-----
[EVENT]:esc
line 1
foo bar
line 3
{rev}lines 1-3/3 100%{/rev}
-----
[EVENT]:char:q
";
    assert_eq!(screen.out(), want);
}

// Once a preview is showing, making the input invalid again (e.g. by appending an
// unmatched `(`) must keep the last valid preview instead of clearing it.
#[test]
fn invalid_regex_after_valid_preserves_last_match() {
    let content = "\
line 1
foo bar
line 3
";
    let screen = run_test_screen(TestCase {
        screen_width: 20,
        screen_height: 4,
        content,
        events: vec![
            key('/'),
            key('f'),
            key('o'),
            key('o'),
            key('('),
            backspace(),
            esc(),
            key('q'),
        ],
        ..Default::default()
    });
    let want = "\
line 1
foo bar
line 3
{rev}lines 1-3/3 100%{/rev}
-----
[EVENT]:char:/
line 1
foo bar
line 3
/█
-----
[EVENT]:char:f
line 1
{rev}{b}f{/rev}{/b}oo bar
line 3
/f█
-----
[EVENT]:char:o
line 1
{rev}{b}fo{/rev}{/b}o bar
line 3
/fo█
-----
[EVENT]:char:o
line 1
{rev}{b}foo{/rev}{/b} bar
line 3
/foo█
-----
[EVENT]:char:(
line 1
{rev}{b}foo{/rev}{/b} bar
line 3
/foo(█
-----
[EVENT]:backspace
line 1
{rev}{b}foo{/rev}{/b} bar
line 3
/foo█
-----
[EVENT]:esc
line 1
foo bar
line 3
{rev}lines 1-3/3 100%{/rev}
-----
[EVENT]:char:q
";
    assert_eq!(screen.out(), want);
}

// Enter must be ignored (staying in search input mode) while the raw input is an
// invalid regex, so a stale/partial query is never committed.
#[test]
fn enter_ignored_while_regex_invalid() {
    let content = "\
line 1
foo bar
line 3
";
    let screen = run_test_screen(TestCase {
        screen_width: 20,
        screen_height: 4,
        content,
        events: vec![
            key('/'),
            key('f'),
            key('o'),
            key('o'),
            key('('),
            enter(), // ignored: input "foo(" is invalid, so nothing is repainted
            esc(),
            key('q'),
        ],
        ..Default::default()
    });
    let want = "\
line 1
foo bar
line 3
{rev}lines 1-3/3 100%{/rev}
-----
[EVENT]:char:/
line 1
foo bar
line 3
/█
-----
[EVENT]:char:f
line 1
{rev}{b}f{/rev}{/b}oo bar
line 3
/f█
-----
[EVENT]:char:o
line 1
{rev}{b}fo{/rev}{/b}o bar
line 3
/fo█
-----
[EVENT]:char:o
line 1
{rev}{b}foo{/rev}{/b} bar
line 3
/foo█
-----
[EVENT]:char:(
line 1
{rev}{b}foo{/rev}{/b} bar
line 3
/foo(█
-----
[EVENT]:enter
[EVENT]:esc
line 1
foo bar
line 3
{rev}lines 1-3/3 100%{/rev}
-----
[EVENT]:char:q
";
    assert_eq!(screen.out(), want);
}

// A zero-width-matching pattern (`a*`) must not hang the incremental search.
#[test]
fn zero_width_pattern_does_not_hang() {
    let content = "\
bbb
aaa
line 3
";
    let screen = run_test_screen(TestCase {
        screen_width: 20,
        screen_height: 4,
        content,
        events: vec![key('/'), key('a'), key('*'), esc(), key('q')],
        ..Default::default()
    });
    let want = "\
bbb
aaa
line 3
{rev}lines 1-3/3 100%{/rev}
-----
[EVENT]:char:/
bbb
aaa
line 3
/█
-----
[EVENT]:char:a
bbb
{rev}{b}a{/rev}{/b}{rev}{line}{b}a{/rev}{/line}{/b}{rev}{line}{b}a{/rev}{/line}{/b}
line 3
/a█
-----
[EVENT]:char:*
bbb
{rev}{b}aaa{/rev}{/b}
line 3
/a*█
-----
[EVENT]:esc
bbb
aaa
line 3
{rev}lines 1-3/3 100%{/rev}
-----
[EVENT]:char:q
";
    assert_eq!(screen.out(), want);
}
