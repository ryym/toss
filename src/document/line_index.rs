use std::fs::File;
use std::io::{self, BufRead, BufReader, Read};
use std::path::Path;

/// Index of byte offsets for each line in a file.
/// Enables O(1) lookup of any line's byte range without loading the content.
pub struct LineIndex {
    /// `offsets[i]` is the byte offset where line `i` starts.
    /// `offsets[line_count]` is the total file size (sentinel),
    /// so that the byte range for line `i` is `offsets[i]..offsets[i+1]`.
    offsets: Vec<u64>,
}

impl LineIndex {
    /// Build a line index by scanning a file for newline positions.
    pub fn from_file(path: &Path) -> io::Result<Self> {
        let file = File::open(path)?;
        Self::from_reader(file)
    }

    /// Build a line index by scanning a reader for newline positions.
    fn from_reader(reader: impl Read) -> io::Result<Self> {
        let mut offsets = vec![0u64];
        let mut pos: u64 = 0;
        let mut reader = BufReader::new(reader);

        loop {
            let buf = reader.fill_buf()?;
            if buf.is_empty() {
                break;
            }
            for &byte in buf {
                pos += 1;
                if byte == b'\n' {
                    offsets.push(pos);
                }
            }
            let len = buf.len();
            reader.consume(len);
        }

        if pos == 0 {
            // Empty content, no lines. offsets = [0] acts as sentinel only.
            return Ok(Self { offsets });
        }

        // If the file ends with a newline, the last offset points right after it.
        // That would create a phantom empty trailing line. Remove it to match
        // `str::lines()` behavior, and adjust the sentinel so the last line's
        // byte range excludes the trailing newline.
        let has_trailing_newline = offsets.last() == Some(&pos);
        if has_trailing_newline {
            offsets.pop();
        }
        let sentinel = if has_trailing_newline { pos - 1 } else { pos };
        offsets.push(sentinel);

        Ok(Self { offsets })
    }

    /// Total number of lines.
    pub fn line_count(&self) -> usize {
        // offsets has line_count + 1 elements (including the sentinel).
        self.offsets.len() - 1
    }

    /// Byte range `(start, end)` for the given line index.
    /// The range excludes the trailing newline character.
    pub fn line_byte_range(&self, index: usize) -> Option<(u64, u64)> {
        let line_count = self.line_count();
        if index >= line_count {
            return None;
        }
        let start = self.offsets[index];
        let end = self.offsets[index + 1];
        // For non-last lines, end points to the byte after '\n'.
        // Strip the newline by subtracting 1.
        // For the last line, the sentinel is already adjusted to exclude
        // any trailing newline, so use it as-is.
        let end = if index < line_count - 1 { end - 1 } else { end };
        Some((start, end))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build(content: &str) -> LineIndex {
        LineIndex::from_reader(content.as_bytes()).unwrap()
    }

    #[test]
    fn empty_content() {
        let idx = build("");
        assert_eq!(idx.line_count(), 0);
        assert_eq!(idx.line_byte_range(0), None);
    }

    #[test]
    fn single_line_no_newline() {
        let idx = build("hello");
        assert_eq!(idx.line_count(), 1);
        assert_eq!(idx.line_byte_range(0), Some((0, 5)));
    }

    #[test]
    fn single_line_with_newline() {
        let idx = build("hello\n");
        assert_eq!(idx.line_count(), 1);
        assert_eq!(idx.line_byte_range(0), Some((0, 5)));
    }

    #[test]
    fn multiple_lines() {
        let idx = build("aaa\nbb\nc\n");
        assert_eq!(idx.line_count(), 3);
        assert_eq!(idx.line_byte_range(0), Some((0, 3))); // "aaa"
        assert_eq!(idx.line_byte_range(1), Some((4, 6))); // "bb"
        assert_eq!(idx.line_byte_range(2), Some((7, 8))); // "c"
    }

    #[test]
    fn multiple_lines_no_trailing_newline() {
        let idx = build("aaa\nbb\nc");
        assert_eq!(idx.line_count(), 3);
        assert_eq!(idx.line_byte_range(0), Some((0, 3))); // "aaa"
        assert_eq!(idx.line_byte_range(1), Some((4, 6))); // "bb"
        assert_eq!(idx.line_byte_range(2), Some((7, 8))); // "c"
    }

    #[test]
    fn empty_lines() {
        let idx = build("\n\n\n");
        assert_eq!(idx.line_count(), 3);
        assert_eq!(idx.line_byte_range(0), Some((0, 0))); // ""
        assert_eq!(idx.line_byte_range(1), Some((1, 1))); // ""
        assert_eq!(idx.line_byte_range(2), Some((2, 2))); // ""
    }

    #[test]
    fn out_of_bounds() {
        let idx = build("a\nb");
        assert_eq!(idx.line_byte_range(2), None);
        assert_eq!(idx.line_byte_range(100), None);
    }
}
