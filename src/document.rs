use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;
use std::sync::mpsc::{Receiver, TryRecvError};

mod line_cache;
mod line_index;

use crate::line::Line;
use line_cache::LineCache;
use line_index::LineIndex;

const LINE_CACHE_CAPACITY: usize = 1000;

/// A message sent from a background reader to a streaming [`Source`].
pub enum StreamMsg {
    /// A batch of newly read lines.
    Lines(Vec<Line>),
    /// The reader reached the end of input.
    Eof,
    /// The reader failed. Treated as end of input.
    Error(io::Error),
}

/// Result of draining pending input in [`Document::pump`].
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PumpResult {
    /// Whether any new lines were appended.
    pub grew: bool,
    /// Whether the input reached its end during this pump.
    pub reached_eof: bool,
}

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
    /// Lines arriving incrementally from a background reader.
    /// Already-received lines are held in `lines`; more may arrive via `rx`.
    Stream {
        lines: Vec<Line>,
        rx: Receiver<StreamMsg>,
        complete: bool,
    },
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

    /// Create a streaming document fed by a channel.
    /// Lines arrive incrementally and are appended on [`Self::pump`].
    pub fn from_channel(rx: Receiver<StreamMsg>) -> Self {
        Self {
            source: Source::Stream {
                lines: Vec::new(),
                rx,
                complete: false,
            },
        }
    }

    /// Drain any pending input into the document without blocking.
    /// For non-streaming sources this is a no-op.
    pub fn pump(&mut self) -> PumpResult {
        let Source::Stream {
            lines,
            rx,
            complete,
        } = &mut self.source
        else {
            return PumpResult::default();
        };
        let mut result = PumpResult::default();
        if *complete {
            return result;
        }
        loop {
            match rx.try_recv() {
                Ok(StreamMsg::Lines(mut batch)) => {
                    if !batch.is_empty() {
                        lines.append(&mut batch);
                        result.grew = true;
                    }
                }
                Ok(StreamMsg::Eof) => {
                    *complete = true;
                    result.reached_eof = true;
                    break;
                }
                Ok(StreamMsg::Error(err)) => {
                    log::warn!("Error reading streamed input: {err}");
                    *complete = true;
                    result.reached_eof = true;
                    break;
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    // The reader dropped the sender without an explicit Eof.
                    *complete = true;
                    result.reached_eof = true;
                    break;
                }
            }
        }
        result
    }

    /// Whether all input has been read.
    /// Always true for file and in-memory sources.
    pub fn is_complete(&self) -> bool {
        match &self.source {
            Source::File { .. } | Source::InMemory { .. } => true,
            Source::Stream { complete, .. } => *complete,
        }
    }

    /// Get a line by index. Returns None if out of bounds.
    /// For file-backed documents, loads from disk and caches on miss.
    pub fn line(&mut self, index: usize) -> Option<&Line> {
        match &mut self.source {
            Source::InMemory { lines } => lines.get(index),
            Source::Stream { lines, .. } => lines.get(index),
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
            Source::Stream { lines, .. } => lines.len(),
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
    use std::sync::mpsc;

    fn lines_msg(indices_and_text: &[(usize, &str)]) -> StreamMsg {
        StreamMsg::Lines(
            indices_and_text
                .iter()
                .map(|(i, s)| Line::new(*i, s.to_string()))
                .collect(),
        )
    }

    #[test]
    fn stream_is_incomplete_until_eof() {
        let (tx, rx) = mpsc::channel();
        let mut doc = Document::from_channel(rx);
        assert!(!doc.is_complete());
        assert_eq!(doc.line_count(), 0);

        // Nothing sent yet: pump is a no-op.
        assert_eq!(doc.pump(), PumpResult::default());
        assert!(!doc.is_complete());

        tx.send(lines_msg(&[(0, "a"), (1, "b")])).unwrap();
        let r = doc.pump();
        assert!(r.grew);
        assert!(!r.reached_eof);
        assert_eq!(doc.line_count(), 2);
        assert_eq!(doc.line(0).unwrap().raw(), "a");
        assert_eq!(doc.line(1).unwrap().raw(), "b");
        assert!(!doc.is_complete());

        tx.send(lines_msg(&[(2, "c")])).unwrap();
        tx.send(StreamMsg::Eof).unwrap();
        let r = doc.pump();
        assert!(r.grew);
        assert!(r.reached_eof);
        assert_eq!(doc.line_count(), 3);
        assert_eq!(doc.line(2).unwrap().raw(), "c");
        assert!(doc.is_complete());

        // Pump after completion is a no-op.
        assert_eq!(doc.pump(), PumpResult::default());
    }

    #[test]
    fn stream_drains_multiple_batches_in_one_pump() {
        let (tx, rx) = mpsc::channel();
        let mut doc = Document::from_channel(rx);
        tx.send(lines_msg(&[(0, "a")])).unwrap();
        tx.send(lines_msg(&[(1, "b")])).unwrap();
        let r = doc.pump();
        assert!(r.grew);
        assert_eq!(doc.line_count(), 2);
    }

    #[test]
    fn stream_error_is_treated_as_eof() {
        let (tx, rx) = mpsc::channel();
        let mut doc = Document::from_channel(rx);
        tx.send(lines_msg(&[(0, "a")])).unwrap();
        tx.send(StreamMsg::Error(io::Error::other("boom"))).unwrap();
        let r = doc.pump();
        assert!(r.grew);
        assert!(r.reached_eof);
        assert_eq!(doc.line_count(), 1);
        assert!(doc.is_complete());
    }

    #[test]
    fn stream_disconnect_without_eof_completes() {
        let (tx, rx) = mpsc::channel();
        let mut doc = Document::from_channel(rx);
        tx.send(lines_msg(&[(0, "a")])).unwrap();
        drop(tx);
        let r = doc.pump();
        assert!(r.grew);
        assert!(r.reached_eof);
        assert_eq!(doc.line_count(), 1);
        assert!(doc.is_complete());
    }

    #[test]
    fn from_string_splits_lines() {
        let mut doc = Document::from_string("hello\nworld\nfoo".into());
        assert_eq!(doc.line_count(), 3);
        assert_eq!(doc.line(0).unwrap().raw(), "hello");
        assert_eq!(doc.line(1).unwrap().raw(), "world");
        assert_eq!(doc.line(2).unwrap().raw(), "foo");
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
        assert_eq!(doc.line(0).unwrap().raw(), "aaa");
        assert_eq!(doc.line(2).unwrap().raw(), "ccc");
        assert_eq!(doc.line(1).unwrap().raw(), "bbb");
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
        assert_eq!(doc.line(0).unwrap().raw(), "hello");
        assert_eq!(doc.line(1).unwrap().raw(), "world");

        std::fs::remove_file(&path).unwrap();
    }
}
