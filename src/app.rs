mod core;
mod mode;
#[cfg(test)]
mod tests;

use std::env;
use std::fs::File;
use std::io::{self, IsTerminal};

use crate::AppResult;
use crate::app::core::Core;
use crate::app::mode::Mode;
use crate::logger::setup_file_logger;
use crate::pager::{PageSize, Pager};
use crate::reader::Reader;
use crate::screen::Screen;
use crate::source::{self, Source};

pub fn run() -> AppResult<()> {
    let mut screen = crate::screen::for_terminal()?;
    let args = env::args().skip(1).collect::<Vec<_>>();
    run_with(&mut screen, args)
}

fn run_with<S: Screen>(screen: &mut S, args: Vec<String>) -> AppResult<()> {
    let _guard = setup_file_logger()?;
    log::debug!("Args: {args:?}");
    let size = screen.size()?;
    let stdin = io::stdin().lock();
    if stdin.is_terminal() || !args.is_empty() {
        let file_path = args.first().unwrap();
        log::debug!("Read file: {file_path}");
        let file = File::open(file_path)?;
        let source = source::as_seekable(file);
        let reader = Reader::new(source);
        let pager = Pager::new(reader, PageSize::new(size.cols(), size.rows()))?;
        App::new(pager).run(screen)?;
    } else {
        log::debug!("Read from stdin");
        let source = source::as_readable(stdin);
        let reader = Reader::new(source);
        let pager = Pager::new(reader, PageSize::new(size.cols(), size.rows()))?;
        App::new(pager).run(screen)?;
    }
    Ok(())
}

struct App<R, Src> {
    pager: Pager<R, Src>,
}

impl<R, Src: Source<R>> App<R, Src> {
    fn new(pager: Pager<R, Src>) -> Self {
        Self { pager }
    }

    fn run<S: Screen>(&mut self, screen: &mut S) -> AppResult<()> {
        let mut core = Core::new(screen, &mut self.pager);
        core.draw_rows()?;
        let mut mode = Mode::view(core);
        loop {
            match mode.run()? {
                NextAction::Exit => break,
                NextAction::NextMode(next_mode) => mode = next_mode,
            }
        }
        Ok(())
    }
}

enum NextAction<'app, S, R, Src> {
    NextMode(Mode<'app, S, R, Src>),
    Exit,
}
