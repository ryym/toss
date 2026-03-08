use std::{thread, time::Duration};

use termion::event::{Event, Key};

use crate::{
    AppResult, SearchDirection,
    app::{Core, Mode, NextAction},
    screen::Screen,
    source::Source,
};

#[derive(Debug)]
struct Scroll {
    pub direction: ScrollDirection,
    pub amount: ScrollAmount,
}

#[derive(Debug)]
enum ScrollDirection {
    Up,
    Down,
}

#[derive(Debug)]
enum ScrollAmount {
    HalfPage,
    OnePage,
}

#[derive(Debug)]
pub(in crate::app) struct ViewMode<'app, S, R, Src> {
    core: Core<'app, S, R, Src>,
}

impl<'app, S: Screen, R, Src: Source<R>> ViewMode<'app, S, R, Src> {
    pub fn new(core: Core<'app, S, R, Src>) -> Self {
        Self { core }
    }

    pub fn run(mut self) -> AppResult<NextAction<'app, S, R, Src>> {
        let event = self.core.next_event()?;
        match event {
            Event::Key(key) => match key {
                Key::Esc => return Ok(NextAction::Exit),
                Key::Char(chr) => match chr {
                    'q' => return Ok(NextAction::Exit),
                    'j' => {
                        self.scroll_down_one_row()?;
                    }
                    'k' => {
                        self.scroll_up_one_row()?;
                    }
                    'g' => {
                        self.scroll_to_start()?;
                    }
                    'G' => {
                        self.scroll_to_end()?;
                    }
                    'd' => {
                        self.scroll(Scroll {
                            direction: ScrollDirection::Down,
                            amount: ScrollAmount::HalfPage,
                        })?;
                    }
                    'u' => {
                        self.scroll(Scroll {
                            direction: ScrollDirection::Up,
                            amount: ScrollAmount::HalfPage,
                        })?;
                    }
                    'f' | ' ' => {
                        self.scroll(Scroll {
                            direction: ScrollDirection::Down,
                            amount: ScrollAmount::OnePage,
                        })?;
                    }
                    'b' => {
                        self.scroll(Scroll {
                            direction: ScrollDirection::Up,
                            amount: ScrollAmount::OnePage,
                        })?;
                    }
                    '/' => {
                        return Ok(NextAction::NextMode(Mode::search(
                            self.core,
                            SearchDirection::Down,
                        )));
                    }
                    '?' => {
                        return Ok(NextAction::NextMode(Mode::search(
                            self.core,
                            SearchDirection::Up,
                        )));
                    }
                    'n' => self.jump_to_next_search_match()?,
                    'N' => self.jump_to_next_search_match_reversed()?,
                    _ => {}
                },
                _ => {
                    panic!("unexpected key")
                }
            },
            _ => {
                panic!("unexpected event")
            }
        }
        Ok(NextAction::NextMode(Mode::View(self)))
    }

    fn scroll_to_start(&mut self) -> AppResult<()> {
        if self.core.pager.scroll_to_start()? {
            self.core.redraw_page()?;
        }
        Ok(())
    }

    fn scroll_to_end(&mut self) -> AppResult<()> {
        if self.core.pager.scroll_to_end()? {
            self.core.redraw_page()?;
        }
        Ok(())
    }

    fn scroll_down_one_row(&mut self) -> AppResult<()> {
        if self.scroll_down_one_row_no_flush()? {
            self.core.screen.flush()?;
        }
        Ok(())
    }

    fn scroll_up_one_row(&mut self) -> AppResult<()> {
        if self.scroll_up_one_row_no_flush()? {
            self.core.screen.flush()?;
        }
        Ok(())
    }

    fn scroll_down_one_row_no_flush(&mut self) -> AppResult<bool> {
        let row_size = self.core.pager.size().rows();
        match self.core.pager.scroll_down_one_row()? {
            None => Ok(false),
            Some(new_line_slice) => {
                self.core.screen.scroll_forward(1)?;
                let row_start = row_size - new_line_slice.row_len();
                self.core.screen.goto(0, row_start)?;
                new_line_slice.write_to(self.core.screen)?;
                Ok(true)
            }
        }
    }

    fn scroll_up_one_row_no_flush(&mut self) -> AppResult<bool> {
        match self.core.pager.scroll_up_one_row()? {
            None => Ok(false),
            Some(new_line_slice) => {
                self.core.screen.scroll_backward(1)?;
                self.core.screen.goto(0, 0)?;
                new_line_slice.write_to(self.core.screen)?;
                Ok(true)
            }
        }
    }

    fn scroll(&mut self, option: Scroll) -> AppResult<()> {
        let n_rows = match option.amount {
            ScrollAmount::HalfPage => self.core.pager.size().rows() / 2,
            ScrollAmount::OnePage => self.core.pager.size().rows(),
        };
        let go_down = matches!(option.direction, ScrollDirection::Down);

        let base_delay = 420.0 / (n_rows as f64 + 2.0);
        for step in 0..n_rows {
            if go_down {
                if !self.scroll_down_one_row_no_flush()? {
                    break;
                }
            } else if !self.scroll_up_one_row_no_flush()? {
                break;
            }
            self.core.screen.flush()?;
            let progress = step as f64 / n_rows as f64;
            let eased_progress = progress.powi(3);
            let delay = (1.0 + base_delay * eased_progress) as u64;
            thread::sleep(Duration::from_millis(delay));
        }
        Ok(())
    }

    fn jump_to_next_search_match(&mut self) -> AppResult<()> {
        let rows_to_scroll = self.core.pager.jump_to_next_search_match()?;
        if rows_to_scroll > 0 {
            self.core.screen.scroll_forward(rows_to_scroll as u16)?;
            let row_size = self.core.pager.size().rows();
            let start_row = row_size.saturating_sub(rows_to_scroll);
            self.core.draw_rows(start_row..)?;
            self.core.screen.flush()?;
        }
        Ok(())
    }

    fn jump_to_next_search_match_reversed(&mut self) -> AppResult<()> {
        let rows_to_scroll = self.core.pager.jump_to_next_search_match_reversed()?;
        if rows_to_scroll > 0 {
            self.core.screen.scroll_backward(rows_to_scroll as u16)?;
            self.core.draw_rows(..rows_to_scroll)?;
            self.core.screen.flush()?;
        }
        Ok(())
    }
}
