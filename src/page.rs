use crate::document::Document;
use crate::header::Header;
use crate::options::Options;
use crate::status_line::StatusLine;
use crate::viewport::Viewport;

/// Bundles the document, header, viewport, and status line — everything the
/// rendering functions need to draw a frame (except search highlight).
pub struct Page {
    pub doc: Document,
    pub header: Header,
    pub viewport: Viewport,
    pub status: StatusLine,
}

impl Page {
    pub fn new(mut doc: Document, options: &Options, width: usize, height: usize) -> Self {
        let header = Header::new(options.header);
        let header_height = header.resolve(&mut doc, width).len();
        let content_height = height.saturating_sub(1).saturating_sub(header_height);
        let viewport = Viewport::new(&mut doc, width, content_height, header.min_top_line());
        let status = StatusLine::new();
        Self {
            doc,
            header,
            viewport,
            status,
        }
    }
}
