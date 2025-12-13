use indoc::indoc;
use pretty_assertions::assert_eq;
use termion::event::{Event, Key};

use crate::AppResult;
use crate::screen::ScreenSize;
use crate::screen::mock::MockScreen;

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
fn search_and_highlight() -> AppResult<()> {
    let (path, _file) = tmpfile(TEXT_SMALL)?;
    let mut screen = MockScreen::new(ScreenSize::new(8, 4));
    screen.set_events(vec![
        // Search by "line".
        Event::Key(Key::Char('/')),
        Event::Key(Key::Char('l')),
        Event::Key(Key::Char('i')),
        Event::Key(Key::Char('n')),
        Event::Key(Key::Char('e')),
        Event::Key(Key::Char('\n')),
        // Navigate up and down.
        Event::Key(Key::Char('j')),
        Event::Key(Key::Char('j')),
        Event::Key(Key::Char('k')),
        Event::Key(Key::Char('k')),
        Event::Key(Key::Char('G')),
        Event::Key(Key::Char('g')),
        // quit.
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
        [EVENT]:char:l
        [EVENT]:char:i
        [EVENT]:char:n
        [EVENT]:char:e
        [EVENT]:char:'\\n'
        *(line)* 1
        *(line)* 2
        *(line)* 3
        *(line)* 4
        -----
        [EVENT]:char:j
        *(line)* 2
        *(line)* 3
        *(line)* 4
        *(line)* 5
        -----
        [EVENT]:char:j
        *(line)* 3
        *(line)* 4
        *(line)* 5
        *(line)* 6
        -----
        [EVENT]:char:k
        *(line)* 2
        *(line)* 3
        *(line)* 4
        *(line)* 5
        -----
        [EVENT]:char:k
        *(line)* 1
        *(line)* 2
        *(line)* 3
        *(line)* 4
        -----
        [EVENT]:char:G
        *(line)* 7
        *(line)* 8
        *(line)* 9
        *(line)* 10
        -----
        [EVENT]:char:g
        *(line)* 1
        *(line)* 2
        *(line)* 3
        *(line)* 4
        -----
        [EVENT]:char:q
    "};
    assert_eq!(screen.out(), want);
    Ok(())
}

#[test]
fn search_by_different_keywords() -> AppResult<()> {
    let (path, _file) = tmpfile(TEXT_SMALL)?;
    let mut screen = MockScreen::new(ScreenSize::new(8, 4));
    screen.set_events(vec![
        // Search by "li".
        Event::Key(Key::Char('/')),
        Event::Key(Key::Char('l')),
        Event::Key(Key::Char('i')),
        Event::Key(Key::Char('\n')),
        // Navigate.
        Event::Key(Key::Char('j')),
        Event::Key(Key::Char('j')),
        // Search by "ne".
        Event::Key(Key::Char('/')),
        Event::Key(Key::Char('n')),
        Event::Key(Key::Char('e')),
        Event::Key(Key::Char('\n')),
        // Navigate.
        Event::Key(Key::Char('j')),
        Event::Key(Key::Char('k')),
        Event::Key(Key::Char('k')),
        Event::Key(Key::Char('k')),
        Event::Key(Key::Char('G')),
        // quit.
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
        [EVENT]:char:l
        [EVENT]:char:i
        [EVENT]:char:'\\n'
        *(li)*ne 1
        *(li)*ne 2
        *(li)*ne 3
        *(li)*ne 4
        -----
        [EVENT]:char:j
        *(li)*ne 2
        *(li)*ne 3
        *(li)*ne 4
        *(li)*ne 5
        -----
        [EVENT]:char:j
        *(li)*ne 3
        *(li)*ne 4
        *(li)*ne 5
        *(li)*ne 6
        -----
        [EVENT]:char:/
        [EVENT]:char:n
        [EVENT]:char:e
        [EVENT]:char:'\\n'
        li*(ne)* 3
        li*(ne)* 4
        li*(ne)* 5
        li*(ne)* 6
        -----
        [EVENT]:char:j
        li*(ne)* 4
        li*(ne)* 5
        li*(ne)* 6
        li*(ne)* 7
        -----
        [EVENT]:char:k
        li*(ne)* 3
        li*(ne)* 4
        li*(ne)* 5
        li*(ne)* 6
        -----
        [EVENT]:char:k
        li*(ne)* 2
        li*(ne)* 3
        li*(ne)* 4
        li*(ne)* 5
        -----
        [EVENT]:char:k
        li*(ne)* 1
        li*(ne)* 2
        li*(ne)* 3
        li*(ne)* 4
        -----
        [EVENT]:char:G
        li*(ne)* 7
        li*(ne)* 8
        li*(ne)* 9
        li*(ne)* 10
        -----
        [EVENT]:char:q
    "};
    assert_eq!(screen.out(), want);
    Ok(())
}

#[test]
fn search_and_highlight_wrapped_line() -> AppResult<()> {
    let (path, _file) = tmpfile(indoc! {"
        line 1
        line 2
        line 3
        01234abcde
        vwxyz98765
    "})?;
    let mut screen = MockScreen::new(ScreenSize::new(6, 3));
    screen.set_events(vec![
        // Search by "cde".
        Event::Key(Key::Char('/')),
        Event::Key(Key::Char('c')),
        Event::Key(Key::Char('d')),
        Event::Key(Key::Char('e')),
        Event::Key(Key::Char('\n')),
        Event::Key(Key::Char('q')),
    ]);
    super::run_with(&mut screen, vec![path])?;

    let want = indoc! {"
        line 1
        line 2
        line 3
        -----
        [EVENT]:char:/
        [EVENT]:char:c
        [EVENT]:char:d
        [EVENT]:char:e
        [EVENT]:char:'\\n'
        01234a>
        b*(cde)*
        vwxyz9
        -----
        [EVENT]:char:q
    "};
    assert_eq!(screen.out(), want);
    Ok(())
}

#[test]
fn search_and_highlight_wrapped_line_over_top() -> AppResult<()> {
    let (path, _file) = tmpfile(indoc! {"
        line 1
        line 2
        line 3
        01234abcde
        vwxyz98765
        line 4
    "})?;
    let mut screen = MockScreen::new(ScreenSize::new(6, 3));
    screen.set_events(vec![
        // Search by "ab".
        Event::Key(Key::Char('/')),
        Event::Key(Key::Char('a')),
        Event::Key(Key::Char('b')),
        Event::Key(Key::Char('\n')),
        // Navigate up and down.
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
        [EVENT]:char:/
        [EVENT]:char:a
        [EVENT]:char:b
        [EVENT]:char:'\\n'
        01234*(a>
        b)*cde
        vwxyz9
        -----
        [EVENT]:char:j
        b)*cde
        vwxyz9>
        8765
        -----
        [EVENT]:char:j
        vwxyz9>
        8765
        line 4
        -----
        [EVENT]:char:k
        *(b)*cde
        vwxyz9>
        8765
        -----
        [EVENT]:char:k
        01234*(a>
        *(b)*cde
        vwxyz9>
        -----
        [EVENT]:char:q
    "};
    assert_eq!(screen.out(), want);
    Ok(())
}

#[test]
fn search_and_highlight_wrapped_line_over_bottom() -> AppResult<()> {
    let (path, _file) = tmpfile(indoc! {"
        line 1
        line 2
        line 3
        01234abcde
        vwxyz98765
        line 4
    "})?;
    let mut screen = MockScreen::new(ScreenSize::new(6, 3));
    screen.set_events(vec![
        // Search by "98".
        Event::Key(Key::Char('/')),
        Event::Key(Key::Char('9')),
        Event::Key(Key::Char('8')),
        Event::Key(Key::Char('\n')),
        // Navigate up and down.
        Event::Key(Key::Char('k')),
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
        [EVENT]:char:/
        [EVENT]:char:9
        [EVENT]:char:8
        [EVENT]:char:'\\n'
        vwxyz*(9>
        8)*765
        line 4
        -----
        [EVENT]:char:k
        bcde
        vwxyz*(9>
        8)*765
        -----
        [EVENT]:char:k
        01234a>
        bcde
        vwxyz*(9>
        -----
        [EVENT]:char:j
        bcde
        vwxyz*(9>
        8)*765
        -----
        [EVENT]:char:j
        vwxyz*(9>
        8)*765
        line 4
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
        line *(9)*
        line 10


        -----
        [EVENT]:char:j
        [EVENT]:char:k
        line 8
        line *(9)*
        line 10

        -----
        [EVENT]:char:j
        [EVENT]:char:k
        line 7
        line 8
        line *(9)*
        line 10
        -----
        [EVENT]:char:k
        line 6
        line 7
        line 8
        line *(9)*
        -----
        [EVENT]:char:j
        line 7
        line 8
        line *(9)*
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
         *(10)*


        -----
        [EVENT]:char:j
        [EVENT]:char:k
         9
        line>
         *(10)*

        -----
        [EVENT]:char:j
        [EVENT]:char:k
        line>
         9
        line>
         *(10)*
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
         *(10)*
        -----
        [EVENT]:char:q
    "};
    assert_eq!(screen.out(), want);

    Ok(())
}

#[test]
fn search_and_jump_matches_forward() -> AppResult<()> {
    let (path, _file) = tmpfile(indoc! {"
        apple 1
        orange
        banana
        apple 2
        orange
        banana
        orange
        apple 3
        banana
        orange
        apple 4
        banana
    "})?;
    let mut screen = MockScreen::new(ScreenSize::new(10, 3));
    screen.set_events(vec![
        // Search by "apple".
        Event::Key(Key::Char('/')),
        Event::Key(Key::Char('a')),
        Event::Key(Key::Char('p')),
        Event::Key(Key::Char('p')),
        Event::Key(Key::Char('l')),
        Event::Key(Key::Char('e')),
        Event::Key(Key::Char('\n')),
        // Jump to subsequent matches.
        Event::Key(Key::Char('n')),
        Event::Key(Key::Char('n')),
        // Move and then jump to a next match.
        Event::Key(Key::Char('k')),
        Event::Key(Key::Char('k')),
        Event::Key(Key::Char('n')),
        Event::Key(Key::Char('q')),
    ]);
    super::run_with(&mut screen, vec![path])?;

    let want = indoc! {"
        apple 1
        orange
        banana
        -----
        [EVENT]:char:/
        [EVENT]:char:a
        [EVENT]:char:p
        [EVENT]:char:p
        [EVENT]:char:l
        [EVENT]:char:e
        [EVENT]:char:'\\n'
        *(apple)* 1
        orange
        banana
        -----
        [EVENT]:char:n
        *(apple)* 2
        orange
        banana
        -----
        [EVENT]:char:n
        *(apple)* 3
        banana
        orange
        -----
        [EVENT]:char:k
        orange
        *(apple)* 3
        banana
        -----
        [EVENT]:char:k
        banana
        orange
        *(apple)* 3
        -----
        [EVENT]:char:n
        *(apple)* 3
        banana
        orange
        -----
        [EVENT]:char:q
    "};
    assert_eq!(screen.out(), want);
    Ok(())
}

#[test]
fn search_and_jump_matches_back_and_forth() -> AppResult<()> {
    let (path, _file) = tmpfile(indoc! {"
        apple 1
        orange
        banana
        apple 2
        orange
        banana
        orange
        apple 3
        banana
        orange
        apple 4
        banana
    "})?;
    let mut screen = MockScreen::new(ScreenSize::new(10, 3));
    screen.set_events(vec![
        // Search by "apple".
        Event::Key(Key::Char('/')),
        Event::Key(Key::Char('a')),
        Event::Key(Key::Char('p')),
        Event::Key(Key::Char('p')),
        Event::Key(Key::Char('l')),
        Event::Key(Key::Char('e')),
        Event::Key(Key::Char('\n')),
        // Jump forward through matches.
        Event::Key(Key::Char('n')),
        Event::Key(Key::Char('n')),
        // Jump backward through matches.
        Event::Key(Key::Char('N')),
        Event::Key(Key::Char('N')),
        Event::Key(Key::Char('q')),
    ]);
    super::run_with(&mut screen, vec![path])?;

    let want = indoc! {"
        apple 1
        orange
        banana
        -----
        [EVENT]:char:/
        [EVENT]:char:a
        [EVENT]:char:p
        [EVENT]:char:p
        [EVENT]:char:l
        [EVENT]:char:e
        [EVENT]:char:'\\n'
        *(apple)* 1
        orange
        banana
        -----
        [EVENT]:char:n
        *(apple)* 2
        orange
        banana
        -----
        [EVENT]:char:n
        *(apple)* 3
        banana
        orange
        -----
        [EVENT]:char:N
        *(apple)* 2
        orange
        banana
        -----
        [EVENT]:char:N
        *(apple)* 1
        orange
        banana
        -----
        [EVENT]:char:q
    "};
    assert_eq!(screen.out(), want);
    Ok(())
}

#[test]
fn search_backward_and_jump_matches() -> AppResult<()> {
    let (path, _file) = tmpfile(indoc! {"
        apple 1
        orange
        banana
        apple 2
        orange
        banana
        orange
        apple 3
        banana
        orange
        apple 4
        banana
    "})?;
    let mut screen = MockScreen::new(ScreenSize::new(10, 3));
    screen.set_events(vec![
        // Move to bottom and then up to make apple 3 visible.
        Event::Key(Key::Char('G')),
        Event::Key(Key::Char('k')),
        Event::Key(Key::Char('k')),
        Event::Key(Key::Char('k')),
        // Search backward by "apple".
        Event::Key(Key::Char('?')),
        Event::Key(Key::Char('a')),
        Event::Key(Key::Char('p')),
        Event::Key(Key::Char('p')),
        Event::Key(Key::Char('l')),
        Event::Key(Key::Char('e')),
        Event::Key(Key::Char('\n')),
        // Jump to previous matches (upward).
        Event::Key(Key::Char('n')),
        Event::Key(Key::Char('n')),
        // Jump to next match (downward).
        Event::Key(Key::Char('N')),
        Event::Key(Key::Char('q')),
    ]);
    super::run_with(&mut screen, vec![path])?;

    let want = indoc! {"
        apple 1
        orange
        banana
        -----
        [EVENT]:char:G
        orange
        apple 4
        banana
        -----
        [EVENT]:char:k
        banana
        orange
        apple 4
        -----
        [EVENT]:char:k
        apple 3
        banana
        orange
        -----
        [EVENT]:char:k
        orange
        apple 3
        banana
        -----
        [EVENT]:char:?
        [EVENT]:char:a
        [EVENT]:char:p
        [EVENT]:char:p
        [EVENT]:char:l
        [EVENT]:char:e
        [EVENT]:char:'\\n'
        *(apple)* 3
        banana
        orange
        -----
        [EVENT]:char:n
        *(apple)* 2
        orange
        banana
        -----
        [EVENT]:char:n
        *(apple)* 1
        orange
        banana
        -----
        [EVENT]:char:N
        *(apple)* 2
        orange
        banana
        -----
        [EVENT]:char:q
    "};
    assert_eq!(screen.out(), want);
    Ok(())
}
