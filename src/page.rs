use crate::document::Document;
use crate::status_line::StatusLine;
use crate::viewport::Viewport;

/// Bundles the document, viewport, and status line — everything the
/// rendering functions need to draw a frame (except search highlight).
pub struct Page {
    pub doc: Document,
    pub viewport: Viewport,
    pub status: StatusLine,
}
