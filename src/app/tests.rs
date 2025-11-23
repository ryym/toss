use indoc::indoc;
use pretty_assertions::assert_eq;

use crate::AppResult;
use crate::screen::mock::MockScreen;
use crate::screen::{Event, Key, ScreenSize};

fn tmpfile(content: &str) -> AppResult<(String, tempfile::NamedTempFile)> {
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
fn open_and_quit() -> AppResult<()> {
    let (path, _file) = tmpfile(TEXT_SMALL)?;
    let mut screen = MockScreen::new(ScreenSize::new(10, 3));
    screen.set_events(vec![Event::Key(Key::Char('q'))]);
    super::run_with(&mut screen, vec![path])?;

    let want = indoc! {"
        line 1
        line 2
        line 3
        -----
        [EVENT]:char:q
    "};
    assert_eq!(screen.out(), want);
    Ok(())
}

#[test]
fn navigate_up_down() -> AppResult<()> {
    let (path, _file) = tmpfile(TEXT_SMALL)?;
    let mut screen = MockScreen::new(ScreenSize::new(10, 3));
    screen.set_events(vec![
        Event::Key(Key::Char('j')),
        Event::Key(Key::Char('j')),
        Event::Key(Key::Char('k')),
        Event::Key(Key::Char('k')),
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
        [EVENT]:char:k
        line 1
        line 2
        line 3
        -----
        [EVENT]:char:q
    "};
    assert_eq!(screen.out(), want);
    Ok(())
}

#[test]
fn navigate_top_bottom() -> AppResult<()> {
    let (path, _file) = tmpfile(TEXT_SMALL)?;
    let mut screen = MockScreen::new(ScreenSize::new(10, 3));
    screen.set_events(vec![
        Event::Key(Key::Char('G')),
        Event::Key(Key::Char('g')),
        Event::Key(Key::Char('G')),
        Event::Key(Key::Char('q')),
    ]);
    super::run_with(&mut screen, vec![path])?;

    let want = indoc! {"
        line 1
        line 2
        line 3
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
        [EVENT]:char:G
        line 8
        line 9
        line 10
        -----
        [EVENT]:char:q
    "};
    assert_eq!(screen.out(), want);

    Ok(())
}

#[test]
fn cannot_go_beyond_top() -> AppResult<()> {
    let (path, _file) = tmpfile(TEXT_SMALL)?;
    let mut screen = MockScreen::new(ScreenSize::new(10, 3));
    screen.set_events(vec![
        Event::Key(Key::Char('j')),
        Event::Key(Key::Char('k')),
        Event::Key(Key::Char('k')),
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
        [EVENT]:char:k
        line 1
        line 2
        line 3
        -----
        [EVENT]:char:k
        [EVENT]:char:g
        [EVENT]:char:q
    "};
    assert_eq!(screen.out(), want);
    Ok(())
}

#[test]
fn cannot_go_beyond_bottom() -> AppResult<()> {
    let (path, _file) = tmpfile(TEXT_SMALL)?;
    let mut screen = MockScreen::new(ScreenSize::new(10, 3));
    screen.set_events(vec![
        Event::Key(Key::Char('G')),
        Event::Key(Key::Char('j')),
        Event::Key(Key::Char('k')),
        Event::Key(Key::Char('j')),
        Event::Key(Key::Char('j')),
        Event::Key(Key::Char('q')),
    ]);
    super::run_with(&mut screen, vec![path])?;

    let want = indoc! {"
        line 1
        line 2
        line 3
        -----
        [EVENT]:char:G
        line 8
        line 9
        line 10
        -----
        [EVENT]:char:j
        [EVENT]:char:k
        line 7
        line 8
        line 9
        -----
        [EVENT]:char:j
        line 8
        line 9
        line 10
        -----
        [EVENT]:char:j
        [EVENT]:char:q
    "};
    assert_eq!(screen.out(), want);
    Ok(())
}

#[test]
fn smooth_scroll_up_down() -> AppResult<()> {
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
    assert_eq!(screen.out(), want);

    Ok(())
}

#[test]
fn navigate_up_down_wrapped_lines() -> AppResult<()> {
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
        [EVENT]:char:q
    "};
    assert_eq!(screen.out(), want);

    Ok(())
}

#[test]
fn navigate_top_bottom_wrapped_lines() -> AppResult<()> {
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
    assert_eq!(screen.out(), want);

    Ok(())
}

#[test]
fn navigate_top_wrapped_lines() -> AppResult<()> {
    let (path, _file) = tmpfile(indoc! {"
        01234567
        abcdefg
        01234567
    "})?;

    let mut screen = MockScreen::new(ScreenSize::new(5, 3));
    screen.set_events(vec![
        Event::Key(Key::Char('j')),
        Event::Key(Key::Char('g')),
        Event::Key(Key::Char('g')),
        Event::Key(Key::Char('q')),
    ]);
    let args = vec![path];
    super::run_with(&mut screen, args)?;

    let want = indoc! {"
        01234>
        567
        abcde
        -----
        [EVENT]:char:j
        567
        abcde>
        fg
        -----
        [EVENT]:char:g
        01234>
        567
        abcde
        -----
        [EVENT]:char:g
        [EVENT]:char:q
    "};
    assert_eq!(screen.out(), want);

    Ok(())
}

#[test]
fn navigate_over_wrapped_lines_only_on_start_or_end() -> AppResult<()> {
    let (path, _file) = tmpfile(indoc! {"
        0
        01
        012
        0123
        01234567
        012
        01
        0
    "})?;

    let mut screen = MockScreen::new(ScreenSize::new(4, 4));
    screen.set_events(vec![
        Event::Key(Key::Char('j')),
        Event::Key(Key::Char('j')),
        Event::Key(Key::Char('G')),
        Event::Key(Key::Char('k')),
        Event::Key(Key::Char('q')),
    ]);
    let args = vec![path];
    super::run_with(&mut screen, args)?;

    let want = indoc! {"
        0
        01
        012
        0123
        -----
        [EVENT]:char:j
        01
        012
        0123
        0123
        -----
        [EVENT]:char:j
        012
        0123
        0123>
        4567
        -----
        [EVENT]:char:G
        4567
        012
        01
        0
        -----
        [EVENT]:char:k
        0123>
        4567
        012
        01
        -----
        [EVENT]:char:q
    "};
    assert_eq!(screen.out(), want);

    Ok(())
}

#[test]
fn go_beyond_bottom_by_search() -> AppResult<()> {
    let (path, _file) = tmpfile(TEXT_SMALL)?;
    let mut screen = MockScreen::new(ScreenSize::new(10, 4));
    screen.set_events(vec![
        // Jump to the line 9.
        Event::Key(Key::Char('/')),
        Event::Key(Key::Char('9')),
        Event::Key(Key::Char('\n')),
        // The page already past the bottom so cannot scroll down further.
        Event::Key(Key::Char('j')),
        // But it is possible to scroll up.
        Event::Key(Key::Char('k')),
        // Until the page backs above the bottom, scrolling down does not work.
        Event::Key(Key::Char('j')),
        Event::Key(Key::Char('k')),
        Event::Key(Key::Char('k')),
        Event::Key(Key::Char('j')),
        Event::Key(Key::Char('q')),
    ]);
    super::run_with(&mut screen, vec![path])?;

    let want = indoc! {"
        line 1
        line 2
        line 3
        line 4
        -----
        [EVENT]:char:/
        [EVENT]:char:9
        [EVENT]:char:'\\n'
        line 9
        line 10


        -----
        [EVENT]:char:j
        [EVENT]:char:k
        line 8
        line 9
        line 10

        -----
        [EVENT]:char:j
        [EVENT]:char:k
        line 7
        line 8
        line 9
        line 10
        -----
        [EVENT]:char:k
        line 6
        line 7
        line 8
        line 9
        -----
        [EVENT]:char:j
        line 7
        line 8
        line 9
        line 10
        -----
        [EVENT]:char:q
    "};
    assert_eq!(screen.out(), want);

    Ok(())
}

#[test]
fn go_beyond_bottom_by_search_wrapped() -> AppResult<()> {
    let (path, _file) = tmpfile(TEXT_SMALL)?;
    let mut screen = MockScreen::new(ScreenSize::new(4, 4));
    screen.set_events(vec![
        // Jump to the line 10.
        Event::Key(Key::Char('/')),
        Event::Key(Key::Char('1')),
        Event::Key(Key::Char('0')),
        Event::Key(Key::Char('\n')),
        // The page already past the bottom so cannot scroll down further.
        Event::Key(Key::Char('j')),
        // But it is possible to scroll up.
        Event::Key(Key::Char('k')),
        // Until the page backs above the bottom, scrolling down does not work.
        Event::Key(Key::Char('j')),
        Event::Key(Key::Char('k')),
        Event::Key(Key::Char('k')),
        Event::Key(Key::Char('j')),
        Event::Key(Key::Char('q')),
    ]);
    super::run_with(&mut screen, vec![path])?;

    let want = indoc! {"
        line>
         1
        line>
         2
        -----
        [EVENT]:char:/
        [EVENT]:char:1
        [EVENT]:char:0
        [EVENT]:char:'\\n'
        line>
         10


        -----
        [EVENT]:char:j
        [EVENT]:char:k
         9
        line>
         10

        -----
        [EVENT]:char:j
        [EVENT]:char:k
        line>
         9
        line>
         10
        -----
        [EVENT]:char:k
         8
        line>
         9
        line>
        -----
        [EVENT]:char:j
        line>
         9
        line>
         10
        -----
        [EVENT]:char:q
    "};
    assert_eq!(screen.out(), want);

    Ok(())
}
