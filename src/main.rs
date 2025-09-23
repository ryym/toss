use std::error::Error;
use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::io::{IsTerminal, Write};
use std::time::Duration;
use std::{cmp, env, panic, thread};

use termion::cursor::Goto;
use termion::event::{Event, Key};
use termion::input::TermRead;
use termion::raw::IntoRawMode;
use termion::screen::IntoAlternateScreen;
use termion::terminal_size;

fn main() {
    let original_hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        let message = if let Some(s) = panic_info.payload().downcast_ref::<String>() {
            s.as_str()
        } else if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
            s
        } else {
            "Unknown panic message"
        };
        let info = format!("{}\n{:?}", message, panic_info);
        let mut log_file = std::fs::File::create("toss-panic.log").unwrap();
        let _ = log_file.write_all(info.as_bytes());
        original_hook(panic_info);
    }));

    let result = run();
    println!("result {:?}", result);
}

type AnyError = Box<dyn Error>;

fn run() -> Result<(), AnyError> {
    let stdin = io::stdin().lock();
    let lines: Vec<String> = if stdin.is_terminal() {
        let mut args = env::args();
        let file_path = args.nth(1).unwrap();
        let file = File::open(file_path)?;
        let reader = BufReader::new(file);
        reader.lines().map(|l| l.unwrap()).collect()
    } else {
        let reader = BufReader::new(stdin);
        reader.lines().map(|l| l.unwrap()).collect()
    };

    let (_term_cols, term_rows) = terminal_size()?;
    let term_rows = term_rows as usize;

    let stdout = io::stdout().lock();
    let stdout = stdout.into_raw_mode()?;
    let mut screen = stdout.into_alternate_screen()?;

    let mut row_start = 0;
    clear_screen(&mut screen)?;
    draw_lines(&mut screen, &lines, row_start, term_rows)?;

    let input_tty = File::open("/dev/tty")?;
    let mut events = input_tty.events();
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
                    'd' => {
                        let half_page = term_rows / 2;
                        let dest = cmp::min(row_start + half_page, lines.len() - 1);
                        smooth_scroll(&mut screen, &lines, &mut row_start, term_rows, dest)?;
                    }
                    'u' => {
                        let half_page = term_rows / 2;
                        let dest = row_start.saturating_sub(half_page);
                        smooth_scroll(&mut screen, &lines, &mut row_start, term_rows, dest)?;
                    }
                    'f' => {
                        let dest = cmp::min(row_start + term_rows, lines.len() - 1);
                        smooth_scroll(&mut screen, &lines, &mut row_start, term_rows, dest)?;
                    }
                    'b' => {
                        let dest = row_start.saturating_sub(term_rows);
                        smooth_scroll(&mut screen, &lines, &mut row_start, term_rows, dest)?;
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

fn smooth_scroll<W: io::Write>(
    mut screen: W,
    lines: &[String],
    row_start: &mut usize,
    row_end: usize,
    dest: usize,
) -> Result<(), AnyError> {
    let total_steps = dest.abs_diff(*row_start);
    let go_down = dest > *row_start;
    let base_delay = 240.0 / (total_steps as f64 + 2.0);
    for step in 0..total_steps {
        let progress = step as f64 / total_steps as f64;
        let eased_progress = progress.powi(3);
        let delay = (1.0 + base_delay * eased_progress) as u64;
        clear_screen(&mut screen)?;
        draw_lines(&mut screen, &lines, *row_start, row_end)?;
        screen.flush()?;
        thread::sleep(Duration::from_millis(delay));
        if go_down {
            *row_start += 1;
        } else {
            *row_start -= 1;
        }
    }
    Ok(())
}
