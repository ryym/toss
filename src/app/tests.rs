use indoc::indoc;
use pretty_assertions::assert_eq;

use crate::screen::mock::MockScreen;
use crate::screen::{Event, Key, ScreenSize};

fn tmpfile(content: &str) -> Result<(String, tempfile::NamedTempFile), crate::app::AnyError> {
    use std::io::{Seek, SeekFrom, Write};
    let mut tmpfile = tempfile::NamedTempFile::new()?;
    tmpfile.write_all(content.as_bytes())?;
    tmpfile.seek(SeekFrom::Start(0))?;
    let path = tmpfile.path().to_str();
    match path {
        None => Err("invalid file path".into()),
        Some(path) => Ok((path.to_string(), tmpfile)),
    }
}

const TEXT_SMALL: &str = indoc! {"
        line 1
        line 2
        line 3
        line 4
        line 5
        line 6
        line 7
        line 8
        line 9
        line 10
"};

#[test]
fn open_and_quit() -> Result<(), super::AnyError> {
    let (path, _file) = tmpfile(TEXT_SMALL)?;
    let mut screen = MockScreen::new(ScreenSize::new(10, 3));
    screen.set_events(vec![Event::Key(Key::Char('q'))]);
    super::run_with2(&mut screen, vec![path])?;

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
fn navigate_up_down() -> Result<(), super::AnyError> {
    let (path, _file) = tmpfile(TEXT_SMALL)?;
    let mut screen = MockScreen::new(ScreenSize::new(10, 3));
    screen.set_events(vec![
        Event::Key(Key::Char('j')),
        Event::Key(Key::Char('j')),
        Event::Key(Key::Char('k')),
        Event::Key(Key::Char('k')),
        Event::Key(Key::Char('q')),
    ]);
    super::run_with2(&mut screen, vec![path])?;

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
        [EVENT]:char:k
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
fn navigate_top_bottom() -> Result<(), super::AnyError> {
    let (path, _file) = tmpfile(TEXT_SMALL)?;
    let mut screen = MockScreen::new(ScreenSize::new(10, 3));
    screen.set_events(vec![
        // xxx: top bottom main に
        Event::Key(Key::Char('j')),
        Event::Key(Key::Char('j')),
        Event::Key(Key::Char('k')),
        Event::Key(Key::Char('G')),
        Event::Key(Key::Char('g')),
        Event::Key(Key::Char('q')),
    ]);
    super::run_with(&mut screen, vec![path])?;

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
fn smooth_scroll_up_down() -> Result<(), super::AnyError> {
    let (path, _file) = tmpfile(TEXT_SMALL)?;
    let mut screen = MockScreen::new(ScreenSize::new(10, 4));
    screen.set_events(vec![
        Event::Key(Key::Char('d')),
        Event::Key(Key::Char('u')),
        Event::Key(Key::Char('q')),
    ]);
    super::run_with(&mut screen, vec![path])?;

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

#[test]
fn navigate_up_down_wrapped_lines() -> Result<(), super::AnyError> {
    let (path, _file) = tmpfile(indoc! {"
        0
        01234567
        0123456789abcd
    "})?;

    let mut screen = MockScreen::new(ScreenSize::new(5, 4));
    screen.set_events(vec![
        Event::Key(Key::Char('j')),
        Event::Key(Key::Char('j')),
        Event::Key(Key::Char('k')),
        Event::Key(Key::Char('k')),
        Event::Key(Key::Char('G')),
        Event::Key(Key::Char('g')),
        Event::Key(Key::Char('q')),
    ]);
    let args = vec![path];
    super::run_with(&mut screen, args)?;

    let want = indoc! {"
        0
        01234>
        567
        01234
        -----
        [EVENT]:char:j
        01234>
        567
        01234>
        56789
        -----
        [EVENT]:char:j
        567
        01234>
        56789>
        abcd
        -----
        [EVENT]:char:k
        01234>
        567
        01234>
        56789>
        -----
        [EVENT]:char:k
        0
        01234>
        567
        01234>
        -----
        [EVENT]:char:G
        567
        01234>
        56789>
        abcd
        -----
        [EVENT]:char:g
        0
        01234>
        567
        01234
        -----
        [EVENT]:char:q
    "};
    assert_eq!(&screen.out, want);

    Ok(())
}

// XXX: tmp or regression

// #[test]
// fn regression_20251101() -> Result<(), super::AnyError> {
//     // let (path, _file) = tmpfile(indoc! {"
//     //     0
//     //     01234567
//     //     0123456789abcd
//     // "})?;

//     let path = "_work/tmp.txt".to_string();

//     let mut screen = MockScreen::new(ScreenSize::new(98, 30));
//     screen.set_events(vec![Event::Key(Key::Char('q'))]);
//     let args = vec![path];
//     super::run_with2(&mut screen, args)?;

//     let lines = screen.out.lines().collect::<Vec<_>>();
//     dbg!(lines);

//     Ok(())
// }
