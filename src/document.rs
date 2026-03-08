use std::fs;
use std::io::{self, Read};
use std::path::Path;

use crate::line::Line;

/// Stores all lines of the document.
/// For now, reads the entire file into memory.
/// Future: lazy loading with block cache.
pub struct Document {
    lines: Vec<Line>,
}

impl Document {
    pub fn from_file(path: &Path) -> io::Result<Self> {
        let content = fs::read_to_string(path)?;
        Ok(Self::from_string(content))
    }

    pub fn from_reader(reader: &mut impl Read) -> io::Result<Self> {
        let mut content = String::new();
        reader.read_to_string(&mut content)?;
        Ok(Self::from_string(content))
    }

    pub fn from_string(content: String) -> Self {
        let lines = content.lines().map(|s| Line::new(s.to_string())).collect();
        Self { lines }
    }

    /// Get a line by index. Returns None if out of bounds.
    pub fn line(&self, index: usize) -> Option<&Line> {
        self.lines.get(index)
    }

    /// Total number of lines in the document.
    pub fn line_count(&self) -> usize {
        self.lines.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_string_splits_lines() {
        let doc = Document::from_string("hello\nworld\nfoo".into());
        assert_eq!(doc.line_count(), 3);
        assert_eq!(doc.line(0).unwrap().text(), "hello");
        assert_eq!(doc.line(1).unwrap().text(), "world");
        assert_eq!(doc.line(2).unwrap().text(), "foo");
    }

    #[test]
    fn from_string_trailing_newline() {
        let doc = Document::from_string("a\nb\n".into());
        // "a\nb\n".lines() yields ["a", "b"] - trailing newline is not an extra line
        assert_eq!(doc.line_count(), 2);
    }

    #[test]
    fn empty_document() {
        let doc = Document::from_string(String::new());
        assert_eq!(doc.line_count(), 0);
        assert!(doc.line(0).is_none());
    }

    #[test]
    fn out_of_bounds() {
        let doc = Document::from_string("one\ntwo".into());
        assert!(doc.line(2).is_none());
        assert!(doc.line(100).is_none());
    }
}
