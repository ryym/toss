mod highlight;

use std::{collections::HashMap, io, ops::Range};

use crossterm::event::Event;

use crate::{
    document::Document,
    line::Row,
    pager::PageSnapshot,
    screen::{Direction, Screen, Scroll},
};

/// A row position in the document: `(line_index, wrap_index)`.
type RowPos = (usize, usize);

/// A run of consecutive screen rows that render one document line.
///
/// The rows of a wrapped line are written as a single continuous string so the terminal
/// treats the breaks between them as soft wraps, which means a group is the smallest unit
/// that can be redrawn.
#[derive(Debug, PartialEq)]
struct PaintedGroup {
    y: usize,
    height: usize,
    /// The text as written to the screen, with search highlights already applied.
    /// `None` when the line could not be read, in which case the rows are only cleared.
    text: Option<String>,
}

/// What one screen row holds, as the identity used to diff two frames.
///
/// `raw` is part of the identity because the same `(line_index, wrap_index)` covers
/// different text at different widths: a reflow can leave the position untouched while
/// the row now has to show more or less of the line.
#[derive(Debug)]
struct PaintedRow {
    pos: RowPos,
    raw: Range<usize>,
    group: usize,
}

/// A whole frame as it should appear on screen: what [`Renderer`] compares against the
/// frame it painted last to decide what actually has to be written.
#[derive(Debug)]
struct PaintedFrame {
    groups: Vec<PaintedGroup>,
    /// One entry per viewport row, sticky rows included.
    rows: Vec<PaintedRow>,
    status: String,
    /// Screen row the status line sits on. Also the number of viewport rows painted:
    /// an under-filled page pulls the status line up and leaves the rest blank.
    status_y: usize,
    /// Viewport height, i.e. the rows below the status line that must stay blank.
    height: usize,
}

impl PaintedFrame {
    fn text_at(&self, y: usize) -> Option<&Option<String>> {
        self.rows.get(y).map(|row| &self.groups[row.group].text)
    }

    /// Whether screen row `y` of this frame is already showing what row `other_y` of
    /// `other` needs, i.e. it can be reused as is.
    fn matches(&self, y: usize, other: &PaintedFrame, other_y: usize) -> bool {
        match (self.rows.get(y), other.rows.get(other_y)) {
            (Some(a), Some(b)) => {
                a.pos == b.pos && a.raw == b.raw && self.text_at(y) == other.text_at(other_y)
            }
            _ => false,
        }
    }
}

/// Renders a [`PageSnapshot`] to the terminal via a [`Screen`].
///
/// `Renderer` is the bridge between [`crate::pager::Pager`], which manages the page state,
/// and [`Screen`], which abstracts terminal writes. Its sole job is to apply the current
/// page state to the screen; it never modifies the page state itself.
///
/// It keeps the frame it painted last and diffs the new one against it in screen
/// coordinates, so the redraw is decided from what the terminal actually shows rather than
/// from what the pager did. See [`plan_shift`] for how the two frames are aligned.
pub struct Renderer<S: Screen> {
    screen: S,
    last: Option<PaintedFrame>,
}

impl<S: Screen> Renderer<S> {
    pub fn new(screen: S) -> Self {
        Self { screen, last: None }
    }

    pub fn into_screen(self) -> S {
        self.screen
    }

    pub fn poll_event(&mut self, timeout: std::time::Duration) -> io::Result<Option<Event>> {
        self.screen.poll_event(timeout)
    }

    /// Apply the given [`PageSnapshot`] to the screen, writing as little as possible.
    ///
    /// The new frame is aligned against the previous one to find how far the screen
    /// content shifted. When it shifted, the terminal is scrolled so the rows that survive
    /// are kept, and only the rows that do not match after the shift are rewritten.
    pub fn render(&mut self, doc: &mut Document, page: PageSnapshot) -> io::Result<()> {
        let frame = build_frame(doc, &page);
        let shift = self
            .last
            .as_ref()
            .map_or(0, |last| plan_shift(last, &frame));
        let dirty = match &self.last {
            Some(last) => dirty_groups(last, &frame, shift),
            // Nothing has been painted yet, so everything is.
            None => (0..frame.groups.len()).collect(),
        };
        log::debug!("render: shift={shift}, dirty groups={}", dirty.len());

        self.screen.begin_sync()?;

        if let Some(scroll) = as_scroll(shift) {
            self.screen.scroll_terminal(&scroll)?;
        }
        for i in dirty {
            let group = &frame.groups[i];
            self.clear_rows(group.y..(group.y + group.height))?;
            if let Some(text) = &group.text {
                self.screen.write_at(group.y, text)?;
            }
        }

        // A scroll drags the rows below the viewport around too, and an under-filled page
        // leaves blank rows below the status line, so clear everything past the content.
        // A page that shrank also has to clear whatever the previous one painted below it.
        let painted_before = self.last.as_ref().map_or(0, |last| last.status_y);
        let blank_end = frame.height.max(painted_before).max(frame.status_y + 1);
        self.clear_rows(frame.status_y..blank_end)?;
        self.screen.write_at(frame.status_y, &frame.status)?;

        self.screen.end_sync()?;
        self.last = Some(frame);
        self.screen.flush()
    }

    fn clear_rows(&mut self, range: Range<usize>) -> io::Result<()> {
        for y in range {
            self.screen.clear_row(y)?;
        }
        Ok(())
    }
}

/// Build the frame the given page should produce on screen.
fn build_frame(doc: &mut Document, page: &PageSnapshot) -> PaintedFrame {
    let sections = [
        (page.header, 0),
        (page.heading, page.header.len()),
        (page.content, page.total_header_height()),
    ];

    let mut groups: Vec<PaintedGroup> = Vec::new();
    let mut rows: Vec<PaintedRow> = Vec::new();
    for (section, base_y) in sections {
        let mut i = 0;
        while i < section.len() {
            let line_index = section[i].line_index();
            let start = i;
            while i < section.len() && section[i].line_index() == line_index {
                i += 1;
            }
            let group = groups.len();
            groups.push(PaintedGroup {
                y: base_y + start,
                height: i - start,
                text: group_text(doc, page, &section[start..i]),
            });
            for row in &section[start..i] {
                rows.push(PaintedRow {
                    pos: (row.line_index(), row.wrap_index()),
                    raw: row.raw_range().clone(),
                    group,
                });
            }
        }
    }

    PaintedFrame {
        groups,
        rows,
        status: page.status_line.clone(),
        status_y: page.viewport_height(),
        height: page.height,
    }
}

/// The text of one soft-wrap group, with search highlights applied.
fn group_text(doc: &mut Document, page: &PageSnapshot, rows: &[Row]) -> Option<String> {
    let line = doc.line(rows[0].line_index())?;
    let raw_range = rows[0].raw_range().start..rows[rows.len() - 1].raw_range().end;
    let text = highlight::apply_highlight_if_matches(page.search, line, raw_range);
    Some(text.into_owned())
}

/// How far the screen content moved between two frames, in screen rows.
/// A positive shift means the content moved up, i.e. the page scrolled down.
///
/// Every row of the new frame votes for the distance at which it finds itself in the old
/// frame, and the winning distance is the one the terminal should be scrolled by. Because
/// the number of rows that need no redraw is exactly the number of votes a shift got,
/// the most voted shift is also the one that leaves the least to redraw — including a
/// shift of zero, which competes on the same footing and means no terminal scroll.
///
/// Voting rather than probing a chosen row is what keeps the sticky rows from deciding the
/// answer: they stay put while the content moves, so they vote for a shift of zero and are
/// simply outvoted, then redrawn along with the rows the scroll exposed.
fn plan_shift(old: &PaintedFrame, new: &PaintedFrame) -> isize {
    let positions: HashMap<RowPos, usize> = old
        .rows
        .iter()
        .enumerate()
        .map(|(y, row)| (row.pos, y))
        .collect();

    let mut votes: HashMap<isize, usize> = HashMap::new();
    for (y, row) in new.rows.iter().enumerate() {
        let Some(&old_y) = positions.get(&row.pos) else {
            continue;
        };
        if old.matches(old_y, new, y) {
            *votes.entry(old_y as isize - y as isize).or_insert(0) += 1;
        }
    }

    // Ties go to the smallest shift, so an unmoved page never scrolls the terminal. The
    // sign breaks a remaining tie between the two directions, which repeated rows can
    // produce: without it the winner would follow the hash map's iteration order.
    votes
        .into_iter()
        .max_by_key(|&(shift, count)| (count, std::cmp::Reverse((shift.abs(), shift))))
        .map_or(0, |(shift, _)| shift)
}

/// The groups of `new` that the screen does not already show once it is scrolled by
/// `shift`. A group is redrawn as a whole because its rows are written as one string.
fn dirty_groups(old: &PaintedFrame, new: &PaintedFrame, shift: isize) -> Vec<usize> {
    let mut dirty = vec![false; new.groups.len()];
    for (y, row) in new.rows.iter().enumerate() {
        let old_y = y as isize + shift;
        let reusable = old_y >= 0 && old.matches(old_y as usize, new, y);
        if !reusable {
            dirty[row.group] = true;
        }
    }
    dirty
        .into_iter()
        .enumerate()
        .filter_map(|(i, is_dirty)| is_dirty.then_some(i))
        .collect()
}

/// Turn a screen shift into the terminal scroll that realizes it.
fn as_scroll(shift: isize) -> Option<Scroll> {
    let direction = if shift > 0 {
        Direction::Down
    } else {
        Direction::Up
    };
    Scroll::new(direction, shift.unsigned_abs())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(rows: &[(usize, &str)], status: &str) -> PaintedFrame {
        let mut groups = Vec::new();
        let mut painted = Vec::new();
        for (y, &(line_index, text)) in rows.iter().enumerate() {
            groups.push(PaintedGroup {
                y,
                height: 1,
                text: Some(text.to_string()),
            });
            painted.push(PaintedRow {
                pos: (line_index, 0),
                raw: 0..text.len(),
                group: y,
            });
        }
        PaintedFrame {
            groups,
            rows: painted,
            status: status.to_string(),
            status_y: rows.len(),
            height: rows.len(),
        }
    }

    #[test]
    fn shift_is_zero_for_an_unchanged_frame() {
        let old = frame(&[(0, "a"), (1, "b"), (2, "c")], "s");
        let new = frame(&[(0, "a"), (1, "b"), (2, "c")], "s");
        assert_eq!(plan_shift(&old, &new), 0);
        assert!(dirty_groups(&old, &new, 0).is_empty());
    }

    #[test]
    fn scrolling_down_shifts_the_content_up() {
        let old = frame(&[(0, "a"), (1, "b"), (2, "c")], "s");
        let new = frame(&[(2, "c"), (3, "d"), (4, "e")], "s");
        // Only "c" survives, one row higher than before.
        assert_eq!(plan_shift(&old, &new), 2);
        // The two rows the scroll exposed are the only ones to draw.
        assert_eq!(dirty_groups(&old, &new, 2), vec![1, 2]);
    }

    #[test]
    fn scrolling_up_shifts_the_content_down() {
        let old = frame(&[(2, "c"), (3, "d"), (4, "e")], "s");
        let new = frame(&[(1, "b"), (2, "c"), (3, "d")], "s");
        assert_eq!(plan_shift(&old, &new), -1);
        assert_eq!(dirty_groups(&old, &new, -1), vec![0]);
    }

    /// The sticky rows do not move while the content scrolls under them, so they vote for
    /// a shift of zero. The content has to outvote them, or the whole page would be
    /// redrawn on every scroll.
    #[test]
    fn sticky_rows_do_not_decide_the_shift() {
        let old = frame(&[(0, "# A"), (3, "c"), (4, "d"), (5, "e")], "s");
        let new = frame(&[(0, "# A"), (4, "d"), (5, "e"), (6, "f")], "s");
        assert_eq!(plan_shift(&old, &new), 1);
        // The sticky row is dragged along by the scroll, so it is redrawn with the new row.
        assert_eq!(dirty_groups(&old, &new, 1), vec![0, 3]);
    }

    /// Repeated rows can make two opposite shifts tie. Either would render correctly, but
    /// which one wins must not depend on the hash map's iteration order.
    #[test]
    fn a_tie_between_two_directions_is_broken_deterministically() {
        let old = frame(&[(0, "a"), (1, "b"), (2, "c")], "s");
        let new = frame(&[(1, "b"), (0, "a"), (9, "z")], "s");
        // One row votes for a shift of 1 and one for -1; the rule picks the negative one.
        assert_eq!(plan_shift(&old, &new), -1);
    }

    /// A shift only wins when it leaves less to redraw than staying put does.
    #[test]
    fn a_shift_that_saves_nothing_loses_to_staying_put() {
        let old = frame(&[(0, "a"), (1, "b"), (2, "c")], "s");
        let new = frame(&[(7, "x"), (8, "y"), (9, "z")], "s");
        assert_eq!(plan_shift(&old, &new), 0);
        assert_eq!(dirty_groups(&old, &new, 0), vec![0, 1, 2]);
    }

    /// A row whose text changed cannot be reused even though it did not move, so a
    /// highlight moving within the page redraws exactly the rows it touched.
    #[test]
    fn changed_text_makes_a_row_dirty_in_place() {
        let old = frame(&[(0, "a"), (1, "hit"), (2, "c")], "s");
        let new = frame(&[(0, "a"), (1, "{rev}hit{/rev}"), (2, "c")], "s");
        assert_eq!(plan_shift(&old, &new), 0);
        assert_eq!(dirty_groups(&old, &new, 0), vec![1]);
    }

    #[test]
    fn a_shift_turns_into_a_terminal_scroll() {
        let down = as_scroll(2).expect("a non-zero shift scrolls");
        assert_eq!(down.direction, Direction::Down);
        assert_eq!(down.num_rows.get(), 2);

        let up = as_scroll(-3).expect("a non-zero shift scrolls");
        assert_eq!(up.direction, Direction::Up);
        assert_eq!(up.num_rows.get(), 3);

        assert!(as_scroll(0).is_none());
    }
}
