use std::env;
use std::error::Error;
use std::io::Write;
use std::io::{self, BufRead, BufReader};

use termion::cursor::Goto;
use termion::event::{Event, Key};
use termion::input::TermRead;
use termion::raw::IntoRawMode;
use termion::screen::IntoAlternateScreen;
use termion::terminal_size;

fn main() {
    let mut args = env::args();
    let file_path = args.nth(1).unwrap();
    run(file_path).unwrap();
}

type AnyError = Box<dyn Error>;

fn run(file_path: String) -> Result<(), AnyError> {
    let file = std::fs::File::open(file_path)?;
    let reader = BufReader::new(file);
    let lines = reader.lines().map(|l| l.unwrap()).collect::<Vec<_>>();

    let (_term_cols, term_rows) = terminal_size()?;
    let term_rows = term_rows as usize;

    let stdout = io::stdout();
    let stdout = stdout.lock().into_raw_mode()?;
    let mut screen = stdout.into_alternate_screen()?;

    let mut row_start = 0;
    clear_screen(&mut screen)?;
    draw_lines(&mut screen, &lines, row_start, term_rows)?;

    let mut events = io::stdin().events();
    loop {
        screen.flush()?;
        let event = events.next().unwrap()?;
        match event {
            Event::Key(key) => match key {
                Key::Esc => return Ok(()),
                Key::Char(chr) => match chr {
                    'q' => return Ok(()),
                    'j' => {
                        if row_start < lines.len() - 1 {
                            row_start += 1;
                            clear_screen(&mut screen)?;
                            draw_lines(&mut screen, &lines, row_start, term_rows)?;
                        }
                    }
                    'k' => {
                        if row_start > 0 {
                            row_start -= 1;
                            clear_screen(&mut screen)?;
                            draw_lines(&mut screen, &lines, row_start, term_rows)?;
                        }
                    }
                    'g' => {
                        clear_screen(&mut screen)?;
                        row_start = 0;
                        draw_lines(&mut screen, &lines, row_start, term_rows)?;
                    }
                    'G' => {
                        clear_screen(&mut screen)?;
                        row_start = lines.len() - term_rows;
                        draw_lines(&mut screen, &lines, row_start, term_rows)?;
                    }
                    _ => continue,
                },
                _ => {
                    panic!("unexpected key")
                }
            },
            _ => {
                panic!("unexpected event")
            }
        }
    }
}

fn clear_screen<W: io::Write>(mut screen: W) -> Result<(), AnyError> {
    write!(screen, "{}{}", termion::clear::All, Goto(1, 1),)?;
    Ok(())
}

fn draw_lines<W: io::Write>(
    mut screen: W,
    lines: &[String],
    row_start: usize,
    row_end: usize,
) -> Result<(), AnyError> {
    for (i, line) in lines.iter().skip(row_start).take(row_end).enumerate() {
        write!(screen, "{}{}", Goto(0, (i + 1) as u16), line)?;
    }
    Ok(())
}
