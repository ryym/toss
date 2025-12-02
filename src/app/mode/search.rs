use regex::Regex;
use termion::event::{Event, Key};

use crate::{
    AppResult,
    app::{Core, Mode, NextAction},
    screen::Screen,
    source::Source,
};

#[derive(Debug)]
struct SearchState {
    input: Vec<char>,
}

impl SearchState {
    fn new() -> Self {
        Self { input: Vec::new() }
    }
}

#[derive(Debug)]
pub(in crate::app) struct SearchMode<'app, S, R, Src> {
    core: Core<'app, S, R, Src>,
    state: SearchState,
}

impl<'app, S: Screen, R, Src: Source<R>> SearchMode<'app, S, R, Src> {
    pub fn new(core: Core<'app, S, R, Src>) -> Self {
        Self {
            core,
            state: SearchState::new(),
        }
    }

    pub fn run(mut self) -> AppResult<NextAction<'app, S, R, Src>> {
        let event = self.core.next_event()?;
        match event {
            Event::Key(key) => match key {
                Key::Esc => return Ok(NextAction::NextMode(Mode::view(self.core))),
                Key::Char(chr) => match chr {
                    '\n' => {
                        let input = self.state.input.iter().collect::<String>();
                        self.search(&input)?;
                        return Ok(NextAction::NextMode(Mode::view(self.core)));
                    }
                    c => {
                        self.state.input.push(c);
                        // interactive search
                        // let input = self.search_state.input.iter().collect::<String>();
                        // self.search(&input)?;
                    }
                },
                _ => {
                    panic!("unexpected key")
                }
            },
            _ => {
                panic!("unexpected event")
            }
        }
        Ok(NextAction::NextMode(Mode::Search(self)))
    }

    fn search(&mut self, input: &str) -> AppResult<()> {
        let query = Regex::new(input)?;
        if self.core.pager.search(query)? {
            self.core.draw_rows()?;
        }
        Ok(())
    }
}
