#[cfg(test)]
mod tests;

use std::fs::File;
use std::io::IsTerminal;
use std::io::{self, BufRead, BufReader};
use std::time::Duration;
use std::{env, panic, thread};

use crate::error::AnyError;
use crate::lines::Line;
use crate::logger;
use crate::pager::{PageSize, Pager};
use crate::reader::Reader;
use crate::screen::Screen;
use crate::screen::{Event, Key};
use crate::source::{self, Source};
use crate::window::Window;

pub fn run() -> Result<(), AnyError> {
    let _guard = logger::setup_file_logger()?;
    let mut screen = crate::screen::for_terminal()?;
    let args = env::args().skip(1).collect::<Vec<_>>();
    // run_with(&mut screen, args)
    run_with2(&mut screen, args)
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
    let lines = lines.into_iter().map(Line::new).collect();
    let window = Window::new(size.n_cols(), size.n_rows(), lines);
    App::new(screen, window).run()
}

fn run_with2<S: Screen>(screen: &mut S, args: Vec<String>) -> Result<(), AnyError> {
    let size = screen.size()?;
    let stdin = io::stdin().lock();
    if stdin.is_terminal() {
        let file_path = args.first().unwrap();
        let file = File::open(file_path)?;
        let source = source::as_seekable(file);
        let reader = Reader::new(source);
        let pager = Pager::new(reader, PageSize::new(size.n_rows(), size.n_cols()))?;
        if let Some(pager) = pager {
            return App2::new(screen, pager).run();
        }
    } else {
        let source = source::as_readable(stdin);
        let reader = Reader::new(source);
        let pager = Pager::new(reader, PageSize::new(size.n_rows(), size.n_cols()))?;
        if let Some(pager) = pager {
            return App2::new(screen, pager).run();
        }
    }
    Ok(())
}

struct App<'s, S: Screen> {
    screen: &'s mut S,
    window: Window,
}

impl<'s, S: Screen> App<'s, S> {
    fn new(screen: &'s mut S, window: Window) -> Self {
        Self { screen, window }
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
                            self.window.seek_to_start();
                            self.draw_rows()?;
                        }
                        'G' => {
                            self.window.seek_to_end();
                            self.draw_rows()?;
                        }
                        'd' => {
                            self.smooth_scroll(self.window.n_rows() / 2, true)?;
                        }
                        'u' => {
                            self.smooth_scroll(self.window.n_rows() / 2, false)?;
                        }
                        'f' | ' ' => {
                            self.smooth_scroll(self.window.n_rows(), true)?;
                        }
                        'b' => {
                            self.smooth_scroll(self.window.n_rows(), false)?;
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
        for row_span in self.window.row_spans() {
            self.screen.draw_at(i_row, row_span.line())?;
            i_row += row_span.size();
        }

        self.screen.flush()?;
        Ok(())
    }

    fn scroll_forward_oneline(&mut self) -> Result<bool, AnyError> {
        if !self.window.scroll_down_one_row() {
            return Ok(false);
        }
        self.screen.scroll_forward(1)?;
        let new_row_span = self.window.end_row_span();
        let row_span_start = self.window.n_rows() - new_row_span.size();
        self.screen.draw_at(row_span_start, new_row_span.line())?;
        Ok(true)
    }

    fn scroll_backword_oneline(&mut self) -> Result<bool, AnyError> {
        if !self.window.scroll_up_one_row() {
            return Ok(false);
        }
        self.screen.scroll_backward(1)?;
        let new_row_span = self.window.start_row_span();
        self.screen.draw_at(0, new_row_span.line())?;
        Ok(true)
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

struct App2<'s, S, R, Src> {
    screen: &'s mut S,
    pager: Pager<R, Src>,
    // Log for debug. Since the app interacts with the terminal in raw mode,
    // we cannot print debug logs to stdout as usual.
    log: String,
}

impl<'s, S: Screen, R, Src: Source<R>> App2<'s, S, R, Src> {
    fn new(screen: &'s mut S, pager: Pager<R, Src>) -> Self {
        Self {
            screen,
            pager,
            log: String::new(),
        }
    }

    fn run(&mut self) -> Result<(), AnyError> {
        let result = self._run();
        if !self.log.is_empty() {
            let _ = std::fs::write("toss-debug.log", &self.log);
        }
        result
    }

    fn _run(&mut self) -> Result<(), AnyError> {
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
                            self.pager.scroll_to_start()?;
                            self.draw_rows()?;
                        }
                        'G' => {
                            self.pager.scroll_to_end()?;
                            self.draw_rows()?;
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
        for row_span in self.pager.row_spans() {
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
