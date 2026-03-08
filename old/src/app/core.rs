use std::ops::{Bound, RangeBounds};

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
        for line_slice in self.pager.line_slices(..) {
            self.screen.goto(0, i_row)?;
            line_slice.write_to(self.screen)?;
            i_row += line_slice.row_len();
        }

        self.screen.flush()?;
        Ok(())
    }

    pub fn draw_rows(&mut self, row_range: impl RangeBounds<usize>) -> AppResult<()> {
        let start_row = match row_range.start_bound() {
            Bound::Unbounded => 0,
            Bound::Included(i) => *i,
            Bound::Excluded(i) => *i + 1,
        };
        let mut current_row = start_row;
        for line_slice in self.pager.line_slices(row_range) {
            self.screen.goto(0, current_row)?;
            line_slice.write_to(self.screen)?;
            current_row += line_slice.row_len();
        }
        Ok(())
    }
}
