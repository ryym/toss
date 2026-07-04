use std::sync::mpsc;

use pretty_assertions::assert_eq;

use super::mock_screen::MockScreen;
use crate::app::App;
use crate::document::{Document, StreamMsg};
use crate::line::Line;
use crate::options::Options;
use crate::pager::Pager;
use crate::screen::ScreenSize;

fn send_lines(tx: &mpsc::Sender<StreamMsg>, start: usize, count: usize) {
    for i in 0..count {
        let line = Line::new(start + i, format!("line{}", start + i));
        tx.send(StreamMsg::Line(line)).unwrap();
    }
}

/// Input that arrives after the pager has started must still be pumped into the
/// page by the event loop and rendered.
#[test]
fn event_loop_renders_input_that_arrives_after_start() {
    let (tx, rx) = mpsc::channel();
    let mut doc = Document::from_channel(rx);

    // One line must be available before constructing the pager.
    send_lines(&tx, 0, 1);
    doc.pump();

    // viewport height = screen_height - 1 = 4.
    let pager = Pager::new(doc, Options::default(), ScreenSize::new(20, 5));
    let screen = MockScreen::new(20, 5);
    let mut app = App::new(screen, pager).unwrap();
    app.set_instant_scroll();

    // The rest of the input becomes available only after the app has started.
    send_lines(&tx, 1, 9);
    tx.send(StreamMsg::Eof).unwrap();

    app.run().unwrap();
    // The whole first screen reflects the streamed lines (0..=3), not just the
    // single line that was available when the pager was constructed.
    let want = "\
line0
line1
line2
line3
{rev}lines 1-4/10 40%{/rev}
-----
";
    assert_eq!(app.into_screen().out(), want);
}

/// A read error that arrives mid-stream is surfaced through the running app so
/// the caller can turn it into a non-zero exit. The already-read lines stay
/// visible, but the status line flags the truncation.
#[test]
fn event_loop_surfaces_read_error_through_the_app() {
    let (tx, rx) = mpsc::channel();
    let mut doc = Document::from_channel(rx);
    send_lines(&tx, 0, 1);
    doc.pump();

    let pager = Pager::new(doc, Options::default(), ScreenSize::new(40, 5));
    let screen = MockScreen::new(40, 5);
    let mut app = App::new(screen, pager).unwrap();
    app.set_instant_scroll();

    // The reader fails after the first line rather than reaching EOF.
    send_lines(&tx, 1, 2);
    tx.send(StreamMsg::Error(std::io::Error::other("boom")))
        .unwrap();

    app.run().unwrap();
    // The error reached the document and remains readable after the session, so
    // run_inner can map it to a non-zero exit.
    assert_eq!(
        app.doc().stream_error().map(|e| e.to_string()),
        Some("boom".into())
    );
    // The already-read lines stay visible; the status flags the truncation.
    let want = "\
line0
line1
line2
{rev}lines 1-3/3 [read error]{/rev}

-----
";
    assert_eq!(app.into_screen().out(), want);
}
