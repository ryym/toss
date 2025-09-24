#[cfg(test)]
mod tests;

use std::error::Error;
use std::fs::File;
use std::io::IsTerminal;
use std::io::{self, BufRead, BufReader};
use std::time::Duration;
use std::{cmp, env, panic, thread};

use crate::screen::Screen;
use crate::screen::{Event, Key};

type AnyError = Box<dyn Error>;

pub fn run() -> Result<(), AnyError> {
    let mut screen = crate::screen::for_terminal()?;
    let args = env::args().skip(1).collect::<Vec<_>>();
    run_with(&mut screen, args)
}

fn run_with<S: Screen>(screen: &mut S, args: Vec<String>) -> Result<(), AnyError> {
    let stdin = io::stdin().lock();
    let lines: Vec<String> = if stdin.is_terminal() {
        let file_path = args.first().unwrap();
        let file = File::open(file_path)?;
        let reader = BufReader::new(file);
        reader.lines().map(|l| l.unwrap()).collect()
    } else {
        let reader = BufReader::new(stdin);
        reader.lines().map(|l| l.unwrap()).collect()
    };

    let size = screen.size()?;
    let mut row_start = 0;
    screen.clear()?;
    screen.draw(take(&lines, row_start, size.n_rows()))?;

    loop {
        screen.flush()?;
        let event = screen.next_event()?;
        let n_rows = screen.size()?.n_rows();
        match event {
            Event::Key(key) => match key {
                Key::Esc => return Ok(()),
                Key::Char(chr) => match chr {
                    'q' => return Ok(()),
                    'j' => {
                        if row_start < lines.len() - 1 {
                            row_start += 1;
                            screen.clear()?;
                            screen.draw(take(&lines, row_start, n_rows))?;
                        }
                    }
                    'k' => {
                        if row_start > 0 {
                            row_start -= 1;
                            screen.clear()?;
                            screen.draw(take(&lines, row_start, n_rows))?;
                        }
                    }
                    'g' => {
                        screen.clear()?;
                        row_start = 0;
                        screen.draw(take(&lines, row_start, n_rows))?;
                    }
                    'G' => {
                        screen.clear()?;
                        row_start = lines.len() - n_rows;
                        screen.draw(take(&lines, row_start, n_rows))?;
                    }
                    'd' => {
                        let half_page = n_rows / 2;
                        let dest = cmp::min(row_start + half_page, lines.len() - 1);
                        smooth_scroll(screen, &lines, &mut row_start, dest)?;
                    }
                    'u' => {
                        let half_page = n_rows / 2;
                        let dest = row_start.saturating_sub(half_page);
                        smooth_scroll(screen, &lines, &mut row_start, dest)?;
                    }
                    'f' => {
                        let dest = cmp::min(row_start + n_rows, lines.len() - 1);
                        smooth_scroll(screen, &lines, &mut row_start, dest)?;
                    }
                    'b' => {
                        let dest = row_start.saturating_sub(n_rows);
                        smooth_scroll(screen, &lines, &mut row_start, dest)?;
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

fn smooth_scroll<S: Screen>(
    screen: &mut S,
    lines: &[String],
    row_start: &mut usize,
    dest: usize,
) -> Result<(), AnyError> {
    let size = screen.size()?;
    let total_steps = dest.abs_diff(*row_start);
    let go_down = dest > *row_start;
    let base_delay = 240.0 / (total_steps as f64 + 2.0);
    for step in 0..total_steps {
        let progress = step as f64 / total_steps as f64;
        let eased_progress = progress.powi(3);
        let delay = (1.0 + base_delay * eased_progress) as u64;
        screen.clear()?;
        screen.draw(take(&lines, *row_start, size.n_rows()))?;
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

fn take(lines: &[String], start: usize, count: usize) -> &[String] {
    let end = cmp::min(lines.len(), start + count);
    &lines[start..end]
}
