#[cfg(test)]
mod tests;

use std::fs::File;
use std::io::{self, IsTerminal};
use std::time::Duration;
use std::{env, panic, thread};

use crate::error::AnyError;
use crate::logger;
use crate::pager::{PageSize, Pager};
use crate::reader::Reader;
use crate::screen::Screen;
use crate::screen::{Event, Key};
use crate::source::{self, Source};

pub fn run() -> Result<(), AnyError> {
    let _guard = logger::setup_file_logger()?;
    let mut screen = crate::screen::for_terminal()?;
    let args = env::args().skip(1).collect::<Vec<_>>();
    run_with(&mut screen, args)
}

fn run_with<S: Screen>(screen: &mut S, args: Vec<String>) -> Result<(), AnyError> {
    let size = screen.size()?;
    let stdin = io::stdin().lock();
    if stdin.is_terminal() || !args.is_empty() {
        let file_path = args.first().unwrap();
        let file = File::open(file_path)?;
        let source = source::as_seekable(file);
        let reader = Reader::new(source);
        let pager = Pager::new(reader, PageSize::new(size.n_rows(), size.n_cols()));
        App::new(screen, pager).run()?;
    } else {
        let source = source::as_readable(stdin);
        let reader = Reader::new(source);
        let pager = Pager::new(reader, PageSize::new(size.n_rows(), size.n_cols()));
        App::new(screen, pager).run()?;
    }
    Ok(())
}

struct App<'s, S, R, Src> {
    screen: &'s mut S,
    pager: Pager<R, Src>,
}

impl<'s, S: Screen, R, Src: Source<R>> App<'s, S, R, Src> {
    fn new(screen: &'s mut S, pager: Pager<R, Src>) -> Self {
        Self { screen, pager }
    }

    fn run(&mut self) -> Result<(), AnyError> {
        self.draw_rows()?;
        loop {
            let event = self.screen.next_event()?;
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
                            if self.pager.scroll_to_start()? {
                                self.draw_rows()?;
                            }
                        }
                        'G' => {
                            if self.pager.scroll_to_end()? {
                                self.draw_rows()?;
                            }
                        }
                        'd' => {
                            self.smooth_scroll(self.pager.size().rows() / 2, true)?;
                        }
                        'u' => {
                            self.smooth_scroll(self.pager.size().rows() / 2, false)?;
                        }
                        'f' | ' ' => {
                            self.smooth_scroll(self.pager.size().rows(), true)?;
                        }
                        'b' => {
                            self.smooth_scroll(self.pager.size().rows(), false)?;
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

    fn draw_rows(&mut self) -> Result<(), AnyError> {
        self.screen.clear()?;

        let mut i_row = 0;
        let mut page = self.pager.page();
        while let Some(row_span) = page.next_row_span()? {
            self.screen.draw_at(i_row, row_span.line())?;
            i_row += row_span.size();
        }
        self.screen.flush()?;
        Ok(())
    }

    fn scroll_forward_oneline(&mut self) -> Result<bool, AnyError> {
        let row_size = self.pager.size().rows();
        match self.pager.scroll_down_one_row()? {
            None => Ok(false),
            Some(new_row_span) => {
                self.screen.scroll_forward(1)?;
                let row_start = row_size - new_row_span.size();
                self.screen.draw_at(row_start, new_row_span.line())?;
                Ok(true)
            }
        }
    }

    fn scroll_backword_oneline(&mut self) -> Result<bool, AnyError> {
        match self.pager.scroll_up_one_row()? {
            None => Ok(false),
            Some(new_row_span) => {
                self.screen.scroll_backward(1)?;
                self.screen.draw_at(0, new_row_span.line())?;
                Ok(true)
            }
        }
    }

    fn smooth_scroll(&mut self, n_rows: usize, go_down: bool) -> Result<(), AnyError> {
        let base_delay = 420.0 / (n_rows as f64 + 2.0);
        for step in 0..n_rows {
            if go_down {
                if !self.scroll_forward_oneline()? {
                    break;
                }
            } else if !self.scroll_backword_oneline()? {
                break;
            }
            self.screen.flush()?;
            let progress = step as f64 / n_rows as f64;
            let eased_progress = progress.powi(3);
            let delay = (1.0 + base_delay * eased_progress) as u64;
            thread::sleep(Duration::from_millis(delay));
        }
        Ok(())
    }
}
