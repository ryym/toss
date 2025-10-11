#[cfg(test)]
mod tests;

use std::error::Error;
use std::fs::{self, File};
use std::io::IsTerminal;
use std::io::{self, BufRead, BufReader};
use std::time::Duration;
use std::{cmp, env, panic, thread};

use crate::screen::Screen;
use crate::screen::{Event, Key};
use crate::wraps::LineWraps;

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
    wraps: LineWraps,
    /// The index of row which is at the top of the screen.
    row_screen_start: usize,
    n_screen_rows: usize,
    // Log for debug. Since the app interacts with the terminal in raw mode,
    // we cannot print debug logs to stdout as usual.
    log: String,
}

impl<'s, S: Screen> App<'s, S> {
    fn new(screen: &'s mut S) -> Self {
        Self {
            screen,
            wraps: LineWraps::new(vec![], 0),
            row_screen_start: 0,
            n_screen_rows: 0,
            log: String::new(),
        }
    }

    fn run(&mut self, args: Vec<String>) -> Result<(), AnyError> {
        let result = self._run(args);
        if !self.log.is_empty() {
            let _ = fs::write("toss-debug.log", &self.log);
        }
        result
    }

    fn _run(&mut self, args: Vec<String>) -> Result<(), AnyError> {
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
        self.row_screen_start = 0;

        let size = self.screen.size()?;
        self.wraps = LineWraps::new(lines, size.n_cols());
        self.n_screen_rows = size.n_rows();
        self.draw_lines()?;

        loop {
            let event = self.screen.next_event()?;
            let size = self.screen.size()?;
            self.n_screen_rows = size.n_rows();
            match event {
                Event::Key(key) => match key {
                    Key::Esc => return Ok(()),
                    Key::Char(chr) => match chr {
                        'q' => return Ok(()),
                        'j' => {
                            if self.scroll_forward_oneline()? {
                                self.screen.flush()?;
                            }
                        }
                        'k' => {
                            if self.scroll_backword_oneline()? {
                                self.screen.flush()?;
                            }
                        }
                        'g' => {
                            self.row_screen_start = 0;
                            self.draw_lines()?;
                        }
                        'G' => {
                            self.row_screen_start = self.wraps.rows_len() - self.n_screen_rows;
                            self.draw_lines()?;
                        }
                        'd' => {
                            let half_page = self.n_screen_rows / 2;
                            let dest = cmp::min(
                                self.row_screen_start + half_page,
                                self.wraps.rows_len() - 1,
                            );
                            self.smooth_scroll(dest)?;
                        }
                        'u' => {
                            let half_page = self.n_screen_rows / 2;
                            let dest = self.row_screen_start.saturating_sub(half_page);
                            self.smooth_scroll(dest)?;
                        }
                        'f' | ' ' => {
                            let dest = cmp::min(self.row_screen_end(), self.wraps.rows_len() - 1);
                            self.smooth_scroll(dest)?;
                        }
                        'b' => {
                            let dest = self.row_screen_start.saturating_sub(self.n_screen_rows);
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

    /// The index of row which is next at the bottom of the screen (exclusive).
    fn row_screen_end(&self) -> usize {
        self.row_screen_start + self.n_screen_rows
    }

    fn draw_lines(&mut self) -> Result<(), AnyError> {
        let row_screen_end = cmp::min(self.wraps.rows_len(), self.row_screen_end());
        self.screen.clear()?;

        let original_lines = self
            .wraps
            .original_lines_iter(self.row_screen_start, row_screen_end);
        let mut i = 0;
        for view in original_lines {
            self.screen.draw_at(i, view.line)?;
            i += view.n_rows;
        }
        self.screen.flush()?;
        Ok(())
    }

    fn scroll_forward_oneline(&mut self) -> Result<bool, AnyError> {
        if self.row_screen_end() >= self.wraps.rows_len() {
            return Ok(false);
        }
        self.screen.scroll_forward(1)?;
        self.row_screen_start += 1;

        // If the previous last row was the end of the original line,
        // just append the first row of the next line.
        // But if the previous last row was the middle of the original line,
        // it overwrites that line until the new end row.
        // This way, we can let the terminal know that
        // the newly appended row is a continuation of the previous row.
        let new_row = self.wraps.row_at(self.row_screen_end() - 1);
        let line_start_row_idx = new_row.index - new_row.line_slice_index;
        let start_row_idx = cmp::max(line_start_row_idx, self.row_screen_start);
        let visible_line = self.wraps.slice_line(start_row_idx, new_row.index + 1);
        let idx_in_screen = start_row_idx - self.row_screen_start;
        self.screen.draw_at(idx_in_screen, visible_line)?;

        Ok(true)
    }

    fn scroll_backword_oneline(&mut self) -> Result<bool, AnyError> {
        if self.row_screen_start == 0 {
            return Ok(false);
        }
        self.screen.scroll_backward(1)?;
        self.row_screen_start -= 1;

        // Scroll up with consideration of line continuations.
        // See the comment of scroll_forward_oneline for details.
        let new_row = self.wraps.row_at(self.row_screen_start);
        let n_remaining_line_slices = new_row.n_line_slices - new_row.line_slice_index - 1;
        let line_end_row_idx = new_row.index + n_remaining_line_slices + 1;
        let end_row_idx = cmp::min(line_end_row_idx, self.row_screen_end());
        let visible_line = self.wraps.slice_line(new_row.index, end_row_idx);
        self.screen.draw_at(0, visible_line)?;

        Ok(true)
    }

    fn smooth_scroll(&mut self, dest: usize) -> Result<(), AnyError> {
        let total_steps = dest.abs_diff(self.row_screen_start);
        let go_down = dest > self.row_screen_start;
        let base_delay = 420.0 / (total_steps as f64 + 2.0);

        for step in 0..total_steps {
            if go_down {
                if !self.scroll_forward_oneline()? {
                    break;
                }
            } else if !self.scroll_backword_oneline()? {
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
