/// Pager behavior options parsed from command-line arguments.
#[derive(Debug, Clone, Default)]
pub struct Options {
    /// Number of lines to pin as a fixed header (0 means no header).
    pub header: usize,
    /// Section header configuration, if enabled.
    pub section: Option<SectionOptions>,
}

/// Configuration for section-based sticky headers.
#[derive(Debug, Clone)]
pub struct SectionOptions {
    /// Regex pattern to identify section start lines.
    pub pattern: regex::Regex,
    /// Number of lines to display as a section header (default 1).
    pub header_lines: usize,
}
