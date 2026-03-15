/// Pager behavior options parsed from command-line arguments.
#[derive(Debug, Clone, Default)]
pub struct Options {
    /// Number of lines to pin as a fixed header (0 means no header).
    pub header: usize,
}
