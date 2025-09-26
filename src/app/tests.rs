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
        line 1
        line 2
        line 3
        -----
        [EVENT]:char:j
        line 2
        line 3
        line 4
        -----
        [EVENT]:char:j
        line 3
        line 4
        line 5
        -----
        [EVENT]:char:k
        line 2
        line 3
        line 4
        -----
        [EVENT]:char:G
        line 8
        line 9
        line 10
        -----
        [EVENT]:char:g
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
fn test_smooth_scroll() -> Result<(), super::AnyError> {
    let mut screen = MockScreen::new(ScreenSize::new(4));
    screen.set_events(vec![
        Event::Key(Key::Char('d')),
        Event::Key(Key::Char('u')),
        Event::Key(Key::Char('q')),
    ]);

    let args = vec!["tests/testdata/small.txt".to_string()];
    super::run_with(&mut screen, args)?;

    // Animate navigations by rendering each step rather than jumping to the destination at once.
    let want = indoc! {"
        line 1
        line 2
        line 3
        line 4
        -----
        [EVENT]:char:d
        line 2
        line 3
        line 4
        line 5
        -----
        line 3
        line 4
        line 5
        line 6
        -----
        [EVENT]:char:u
        line 2
        line 3
        line 4
        line 5
        -----
        line 1
        line 2
        line 3
        line 4
        -----
        [EVENT]:char:q
    "};
    assert_eq!(&screen.out, want);

    Ok(())
}
