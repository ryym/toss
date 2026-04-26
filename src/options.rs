/// Pager behavior options parsed from command-line arguments.
#[derive(Debug, Clone, Default)]
pub struct Options {
    /// Number of lines to pin as a fixed header (0 means no header).
    pub header: usize,
    /// Sticky heading configuration, if enabled.
    pub heading: Option<HeadingOptions>,
    /// Quit automatically if the entire content fits on one screen.
    pub quit_if_one_screen: bool,
}

/// Configuration for sticky per-section headings.
#[derive(Debug, Clone)]
pub struct HeadingOptions {
    /// Regex pattern matching section heading lines.
    pub pattern: regex::Regex,
    /// Number of lines per heading block (default 1).
    pub num_lines: usize,
}
