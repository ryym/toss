use indoc::indoc;
use pretty_assertions::assert_eq;

use crate::screen::mock::MockScreen;
use crate::screen::{Event, Key, ScreenSize};

#[test]
fn test_open_and_quit() -> Result<(), super::AnyError> {
    let mut screen = MockScreen::new(ScreenSize::new(3));
    screen.set_events(vec![Event::Key(Key::Char('q'))]);

    let args = vec!["tests/testdata/small.txt".to_string()];
    super::run_with(&mut screen, args)?;

    let want = indoc! {"
        [CLEAR]
        line 1
        line 2
        line 3
        -----
        [EVENT]:char:q
    "};
    assert_eq!(&screen.out, want);
    Ok(())
}

#[test]
fn test_basic_navigation() -> Result<(), super::AnyError> {
    let mut screen = MockScreen::new(ScreenSize::new(3));
    screen.set_events(vec![
        Event::Key(Key::Char('j')),
        Event::Key(Key::Char('j')),
        Event::Key(Key::Char('k')),
        Event::Key(Key::Char('G')),
        Event::Key(Key::Char('g')),
        Event::Key(Key::Char('q')),
    ]);

    let args = vec!["tests/testdata/small.txt".to_string()];
    super::run_with(&mut screen, args)?;

    let want = indoc! {"
        [CLEAR]
        line 1
        line 2
        line 3
        -----
        [EVENT]:char:j
        [CLEAR]
        line 2
        line 3
        line 4
        -----
        [EVENT]:char:j
        [CLEAR]
        line 3
        line 4
        line 5
        -----
        [EVENT]:char:k
        [CLEAR]
        line 2
        line 3
        line 4
        -----
        [EVENT]:char:G
        [CLEAR]
        line 8
        line 9
        line 10
        -----
        [EVENT]:char:g
        [CLEAR]
        line 1
        line 2
        line 3
        -----
        [EVENT]:char:q
    "};
    assert_eq!(&screen.out, want);

    Ok(())
}
