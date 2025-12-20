use termion::event::Event;

use crate::{AppResult, pager::Pager, screen::Screen, source::Source};

#[derive(Debug)]
pub(super) struct Core<'app, S, R, Src> {
    pub screen: &'app mut S,
    pub pager: &'app mut Pager<R, Src>,
}

impl<'app, S: Screen, R, Src: Source<R>> Core<'app, S, R, Src> {
    pub fn new(screen: &'app mut S, pager: &'app mut Pager<R, Src>) -> Self {
        Self { screen, pager }
    }

    pub fn next_event(&mut self) -> AppResult<Event> {
        let event = self.screen.next_event()?;
        log::debug!("Event: {event:?}");
        Ok(event)
    }

    pub fn redraw_page(&mut self) -> AppResult<()> {
        self.screen.clear()?;

        let mut i_row = 0;
        for line_slice in self.pager.line_slices() {
            self.screen.goto(0, i_row)?;
            line_slice.write_to(self.screen)?;
            i_row += line_slice.row_len();
        }

        self.screen.flush()?;
        Ok(())
    }
}
