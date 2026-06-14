use std::sync::mpsc;

use super::mock_screen::MockScreen;
use crate::app::App;
use crate::document::{Document, StreamMsg};
use crate::line::Line;
use crate::options::Options;
use crate::pager::Pager;
use crate::screen::ScreenSize;

fn lines_msg(start: usize, count: usize) -> StreamMsg {
    StreamMsg::Lines(
        (0..count)
            .map(|i| Line::new(start + i, format!("line{}", start + i)))
            .collect(),
    )
}

/// Input that arrives after the pager has started must still be pumped into the
/// page by the event loop and rendered.
#[test]
fn event_loop_renders_input_that_arrives_after_start() {
    let (tx, rx) = mpsc::channel();
    let mut doc = Document::from_channel(rx);

    // One line must be available before constructing the pager.
    tx.send(lines_msg(0, 1)).unwrap();
    doc.pump();

    // viewport height = screen_height - 1 = 4.
    let pager = Pager::new(doc, Options::default(), ScreenSize::new(20, 5));
    let screen = MockScreen::new(20, 5);
    let mut app = App::new(screen, pager).unwrap();
    app.set_instant_scroll();

    // The rest of the input becomes available only after the app has started.
    tx.send(lines_msg(1, 9)).unwrap();
    tx.send(StreamMsg::Eof).unwrap();

    app.run().unwrap();
    let out = app.into_screen().out();

    // The first screen should show the streamed lines, not just the initial one.
    assert!(out.contains("line0"), "missing line0:\n{out}");
    assert!(out.contains("line3"), "missing line3:\n{out}");
}
