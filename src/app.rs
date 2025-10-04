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
    App::new(screen).run(args)
}

struct App<'s, S: Screen> {
    screen: &'s mut S,
    lines: Vec<String>,
    /// The index of lines which is at the top of the screen.
    start_line_index: usize,
}

impl<'s, S: Screen> App<'s, S> {
    fn new(screen: &'s mut S) -> Self {
        Self {
            screen,
            lines: Vec::new(),
            start_line_index: 0,
        }
    }

    fn run(&mut self, args: Vec<String>) -> Result<(), AnyError> {
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
        self.lines = lines;
        self.start_line_index = 0;

        let size = self.screen.size()?;
        self.draw_lines(size.n_rows())?;

        loop {
            let event = self.screen.next_event()?;
            let n_rows = self.screen.size()?.n_rows();
            match event {
                Event::Key(key) => match key {
                    Key::Esc => return Ok(()),
                    Key::Char(chr) => match chr {
                        'q' => return Ok(()),
                        'j' => {
                            if self.scroll_forward(n_rows, 1)? {
                                self.screen.flush()?;
                            }
                        }
                        'k' => {
                            if self.scroll_backword(1)? {
                                self.screen.flush()?;
                            }
                        }
                        'g' => {
                            self.start_line_index = 0;
                            self.draw_lines(n_rows)?;
                        }
                        'G' => {
                            self.start_line_index = self.lines.len() - n_rows;
                            self.draw_lines(n_rows)?;
                        }
                        'd' => {
                            let half_page = n_rows / 2;
                            let dest =
                                cmp::min(self.start_line_index + half_page, self.lines.len() - 1);
                            self.smooth_scroll(dest)?;
                        }
                        'u' => {
                            let half_page = n_rows / 2;
                            let dest = self.start_line_index.saturating_sub(half_page);
                            self.smooth_scroll(dest)?;
                        }
                        'f' => {
                            let dest =
                                cmp::min(self.start_line_index + n_rows, self.lines.len() - 1);
                            self.smooth_scroll(dest)?;
                        }
                        'b' => {
                            let dest = self.start_line_index.saturating_sub(n_rows);
                            self.smooth_scroll(dest)?;
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

    fn draw_lines(&mut self, n_rows: usize) -> Result<(), AnyError> {
        let row_end = cmp::min(self.lines.len(), self.start_line_index + n_rows);
        self.screen.clear()?;
        let displayed_lines = &self.lines[self.start_line_index..row_end];
        for (i, line) in displayed_lines.iter().enumerate() {
            self.screen.draw_at(i, line)?;
        }
        self.screen.flush()?;
        Ok(())
    }

    fn scroll_forward(&mut self, n_rows: usize, n_steps: u16) -> Result<bool, AnyError> {
        if self.start_line_index + n_rows >= self.lines.len() {
            return Ok(false);
        }
        self.screen.scroll_forward(n_steps)?;
        let next_line = &self.lines[self.start_line_index + n_rows];
        self.screen.draw_at(n_rows - 1, next_line)?;
        self.start_line_index += 1;
        Ok(true)
    }

    fn scroll_backword(&mut self, n_steps: u16) -> Result<bool, AnyError> {
        if self.start_line_index == 0 {
            return Ok(false);
        }
        self.screen.scroll_backward(n_steps)?;
        self.start_line_index -= 1;
        self.screen.draw_at(0, &self.lines[self.start_line_index])?;
        Ok(true)
    }

    fn smooth_scroll(&mut self, dest: usize) -> Result<(), AnyError> {
        let size = self.screen.size()?;
        let total_steps = dest.abs_diff(self.start_line_index);
        let go_down = dest > self.start_line_index;
        let base_delay = 240.0 / (total_steps as f64 + 2.0);

        for step in 0..total_steps {
            if go_down {
                if !self.scroll_forward(size.n_rows(), 1)? {
                    break;
                }
            } else if !self.scroll_backword(1)? {
                break;
            }
            self.screen.flush()?;
            let progress = step as f64 / total_steps as f64;
            let eased_progress = progress.powi(3);
            let delay = (1.0 + base_delay * eased_progress) as u64;
            thread::sleep(Duration::from_millis(delay));
        }
        Ok(())
    }
}
