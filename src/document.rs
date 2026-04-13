use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;

mod line_cache;
mod line_index;

use crate::line::Line;
use line_cache::LineCache;
use line_index::LineIndex;

const LINE_CACHE_CAPACITY: usize = 1000;

/// Source of line data.
enum Source {
    /// Seekable file with lazy line loading.
    File {
        file: File,
        index: LineIndex,
        cache: LineCache,
    },
    /// All lines held in memory (for stdin or small content).
    InMemory { lines: Vec<Line> },
}

/// Provides access to the lines of a document.
/// For file inputs, lines are loaded on demand and cached.
/// For stdin/string inputs, all lines are held in memory.
pub struct Document {
    source: Source,
}

impl Document {
    /// Open a file and build a line index without loading its content.
    pub fn from_file(path: &Path) -> io::Result<Self> {
        let index = LineIndex::from_file(path)?;
        let file = File::open(path)?;
        Ok(Self {
            source: Source::File {
                file,
                index,
                cache: LineCache::new(LINE_CACHE_CAPACITY),
            },
        })
    }

    /// Read all content from a reader into memory.
    pub fn from_reader(reader: &mut impl Read) -> io::Result<Self> {
        let mut content = String::new();
        reader.read_to_string(&mut content)?;
        Ok(Self::from_string(content))
    }

    /// Parse a string into lines, holding everything in memory.
    pub fn from_string(content: String) -> Self {
        let lines = content
            .lines()
            .enumerate()
            .map(|(i, s)| Line::new(i, s.to_string()))
            .collect();
        Self {
            source: Source::InMemory { lines },
        }
    }

    /// Get a line by index. Returns None if out of bounds.
    /// For file-backed documents, loads from disk and caches on miss.
    pub fn line(&mut self, index: usize) -> Option<&Line> {
        match &mut self.source {
            Source::InMemory { lines } => lines.get(index),
            Source::File {
                file,
                index: line_index,
                cache,
                ..
            } => {
                if cache.get(index).is_some() {
                    return cache.get(index);
                }
                let (start, end) = line_index.line_byte_range(index)?;
                let line = read_line_from_file(file, index, start, end).ok()?;
                Some(cache.insert(index, line))
            }
        }
    }

    /// Total number of lines in the document.
    pub fn line_count(&self) -> usize {
        match &self.source {
            Source::InMemory { lines } => lines.len(),
            Source::File { index, .. } => index.line_count(),
        }
    }
}

/// Read a single line from a file at the given byte range.
fn read_line_from_file(file: &mut File, index: usize, start: u64, end: u64) -> io::Result<Line> {
    let len = (end - start) as usize;
    let mut buf = vec![0u8; len];
    file.seek(SeekFrom::Start(start))?;
    file.read_exact(&mut buf)?;
    // Handle possible \r at end (CRLF line endings).
    if buf.last() == Some(&b'\r') {
        buf.pop();
    }
    let raw = String::from_utf8_lossy(&buf).into_owned();
    Ok(Line::new(index, raw))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_string_splits_lines() {
        let mut doc = Document::from_string("hello\nworld\nfoo".into());
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
        let mut doc = Document::from_string(String::new());
        assert_eq!(doc.line_count(), 0);
        assert!(doc.line(0).is_none());
    }

    #[test]
    fn out_of_bounds() {
        let mut doc = Document::from_string("one\ntwo".into());
        assert!(doc.line(2).is_none());
        assert!(doc.line(100).is_none());
    }

    #[test]
    fn from_file_loads_lines_on_demand() {
        let dir = Path::new(".local/test");
        std::fs::create_dir_all(dir).unwrap();
        let path = dir.join("test_lazy_load.txt");
        std::fs::write(&path, "aaa\nbbb\nccc\n").unwrap();

        let mut doc = Document::from_file(&path).unwrap();
        assert_eq!(doc.line_count(), 3);
        assert_eq!(doc.line(0).unwrap().text(), "aaa");
        assert_eq!(doc.line(2).unwrap().text(), "ccc");
        assert_eq!(doc.line(1).unwrap().text(), "bbb");
        assert!(doc.line(3).is_none());

        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn from_file_no_trailing_newline() {
        let dir = Path::new(".local/test");
        std::fs::create_dir_all(dir).unwrap();
        let path = dir.join("test_no_trailing.txt");
        std::fs::write(&path, "hello\nworld").unwrap();

        let mut doc = Document::from_file(&path).unwrap();
        assert_eq!(doc.line_count(), 2);
        assert_eq!(doc.line(0).unwrap().text(), "hello");
        assert_eq!(doc.line(1).unwrap().text(), "world");

        std::fs::remove_file(&path).unwrap();
    }
}
