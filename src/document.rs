use std::fs::File;
use std::io::{self, BufRead, Read, Seek, SeekFrom};
use std::path::Path;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::thread;

mod line_cache;
mod line_index;

use crate::line::Line;
use line_cache::LineCache;
use line_index::LineIndex;

const LINE_CACHE_CAPACITY: usize = 1000;

/// A message sent from a background reader to a streaming [`Source`].
pub enum StreamMsg {
    /// A newly read line.
    Line(Line),
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

/// State of a streaming [`Source`].
enum StreamState {
    /// More lines may arrive from a background reader.
    Receiving(Receiver<StreamMsg>),
    /// All input has been received; no more lines will arrive.
    Complete,
}

/// Source of line data.
enum Source {
    /// Seekable file with lazy line loading.
    File {
        file: File,
        index: LineIndex,
        cache: LineCache,
    },
    /// Lines held in memory, optionally arriving incrementally.
    /// Already-received lines are held in `lines`. `state` tracks whether more
    /// lines may still arrive from a background reader (`Receiving`) or the
    /// content is final (`Complete`), e.g. for a string-backed document.
    Stream {
        lines: Vec<Line>,
        state: StreamState,
    },
}

/// Provides access to the lines of a document.
/// For file inputs, lines are loaded on demand and cached.
/// For stdin/string inputs, lines are held in memory (arriving incrementally
/// for stdin, all at once for strings).
pub struct Document {
    source: Source,
    /// Display name of the document (e.g. the file path). `None` for stdin/string
    /// sources, which have no name.
    name: Option<String>,
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
            name: Some(path.display().to_string()),
        })
    }

    /// Parse a string into lines, holding everything in memory.
    /// The resulting document is complete: no more lines will arrive.
    #[cfg(test)]
    pub fn from_string(content: String) -> Self {
        let lines = content
            .lines()
            .enumerate()
            .map(|(i, s)| Line::new(i, s.to_string()))
            .collect();
        Self {
            source: Source::Stream {
                lines,
                state: StreamState::Complete,
            },
            name: None,
        }
    }

    /// Create a streaming document fed by a channel.
    /// Lines arrive incrementally and are appended on [`Self::pump`].
    pub fn from_channel(rx: Receiver<StreamMsg>) -> Self {
        Self {
            source: Source::Stream {
                lines: Vec::new(),
                state: StreamState::Receiving(rx),
            },
            name: None,
        }
    }

    /// Create a streaming document that reads from `reader` on a background
    /// thread. Returns immediately; lines become available via [`Self::pump`].
    pub fn from_stdin<R: BufRead + Send + 'static>(reader: R) -> Self {
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || read_lines(reader, tx));
        Self::from_channel(rx)
    }

    /// Drain any pending input into the document without blocking.
    /// For non-streaming sources this is a no-op.
    pub fn pump(&mut self) -> PumpResult {
        let Source::Stream { lines, state } = &mut self.source else {
            return PumpResult::default();
        };
        let mut result = PumpResult::default();
        let StreamState::Receiving(rx) = state else {
            return result;
        };
        loop {
            match rx.try_recv() {
                Ok(StreamMsg::Line(line)) => {
                    lines.push(line);
                    result.grew = true;
                }
                Ok(StreamMsg::Eof) => {
                    *state = StreamState::Complete;
                    result.reached_eof = true;
                    break;
                }
                Ok(StreamMsg::Error(err)) => {
                    log::warn!("Error reading streamed input: {err}");
                    *state = StreamState::Complete;
                    result.reached_eof = true;
                    break;
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    // The reader dropped the sender without an explicit Eof.
                    *state = StreamState::Complete;
                    result.reached_eof = true;
                    break;
                }
            }
        }
        result
    }

    /// Display name of the document (e.g. the file path), or `None` for sources
    /// without a name such as stdin.
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// Whether all input has been read.
    /// Always true for file and in-memory sources.
    pub fn is_complete(&self) -> bool {
        match &self.source {
            Source::File { .. } => true,
            Source::Stream { state, .. } => matches!(state, StreamState::Complete),
        }
    }

    /// Get a line by index. Returns None if out of bounds.
    /// For file-backed documents, loads from disk and caches on miss.
    pub fn line(&mut self, index: usize) -> Option<&Line> {
        match &mut self.source {
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
            Source::Stream { lines, .. } => lines.len(),
            Source::File { index, .. } => index.line_count(),
        }
    }
}

/// Read lines from `reader` until EOF, sending each as it is parsed.
/// Runs on a background thread. Stops early if the receiver is dropped.
fn read_lines<R: BufRead>(mut reader: R, tx: Sender<StreamMsg>) {
    let mut index = 0;
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match reader.read_until(b'\n', &mut buf) {
            Ok(0) => {
                let _ = tx.send(StreamMsg::Eof);
                return;
            }
            Ok(_) => {
                // Strip the line terminator, handling CRLF like read_line_from_file.
                if buf.last() == Some(&b'\n') {
                    buf.pop();
                }
                if buf.last() == Some(&b'\r') {
                    buf.pop();
                }
                let raw = String::from_utf8_lossy(&buf).into_owned();
                let line = Line::new(index, raw);
                index += 1;
                if tx.send(StreamMsg::Line(line)).is_err() {
                    // The document was dropped; stop reading.
                    return;
                }
            }
            Err(err) => {
                let _ = tx.send(StreamMsg::Error(err));
                return;
            }
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

    fn send_lines(tx: &mpsc::Sender<StreamMsg>, indices_and_text: &[(usize, &str)]) {
        for (i, s) in indices_and_text {
            tx.send(StreamMsg::Line(Line::new(*i, s.to_string())))
                .unwrap();
        }
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

        send_lines(&tx, &[(0, "a"), (1, "b")]);
        let r = doc.pump();
        assert!(r.grew);
        assert!(!r.reached_eof);
        assert_eq!(doc.line_count(), 2);
        assert_eq!(doc.line(0).unwrap().raw(), "a");
        assert_eq!(doc.line(1).unwrap().raw(), "b");
        assert!(!doc.is_complete());

        send_lines(&tx, &[(2, "c")]);
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
        send_lines(&tx, &[(0, "a")]);
        send_lines(&tx, &[(1, "b")]);
        let r = doc.pump();
        assert!(r.grew);
        assert_eq!(doc.line_count(), 2);
    }

    #[test]
    fn stream_error_is_treated_as_eof() {
        let (tx, rx) = mpsc::channel();
        let mut doc = Document::from_channel(rx);
        send_lines(&tx, &[(0, "a")]);
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
        send_lines(&tx, &[(0, "a")]);
        drop(tx);
        let r = doc.pump();
        assert!(r.grew);
        assert!(r.reached_eof);
        assert_eq!(doc.line_count(), 1);
        assert!(doc.is_complete());
    }

    /// Pump until the document reports completion, with a timeout guard so a
    /// stuck reader thread fails the test instead of hanging forever.
    fn pump_until_complete(doc: &mut Document) {
        let start = std::time::Instant::now();
        while !doc.is_complete() {
            doc.pump();
            if start.elapsed() > std::time::Duration::from_secs(5) {
                panic!("stream did not complete within timeout");
            }
            std::thread::yield_now();
        }
        doc.pump();
    }

    #[test]
    fn from_stdin_reads_all_lines() {
        let mut doc = Document::from_stdin(io::Cursor::new(b"aaa\nbbb\nccc\n".to_vec()));
        pump_until_complete(&mut doc);
        assert_eq!(doc.line_count(), 3);
        assert_eq!(doc.line(0).unwrap().raw(), "aaa");
        assert_eq!(doc.line(1).unwrap().raw(), "bbb");
        assert_eq!(doc.line(2).unwrap().raw(), "ccc");
        assert!(doc.line(3).is_none());
    }

    #[test]
    fn from_stdin_handles_no_trailing_newline_and_crlf() {
        let mut doc = Document::from_stdin(io::Cursor::new(b"hello\r\nworld".to_vec()));
        pump_until_complete(&mut doc);
        assert_eq!(doc.line_count(), 2);
        assert_eq!(doc.line(0).unwrap().raw(), "hello");
        assert_eq!(doc.line(1).unwrap().raw(), "world");
    }

    #[test]
    fn from_string_is_complete_and_pump_is_noop() {
        let mut doc = Document::from_string("a\nb".into());
        assert!(doc.is_complete());
        assert_eq!(doc.line_count(), 2);
        // A string-backed document has no channel; pump must not change it.
        assert_eq!(doc.pump(), PumpResult::default());
        assert_eq!(doc.line_count(), 2);
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
    fn stream_and_string_sources_have_no_name() {
        let (_tx, rx) = mpsc::channel();
        assert!(Document::from_channel(rx).name().is_none());
        assert!(Document::from_string("a\nb".into()).name().is_none());
    }

    #[test]
    fn from_file_exposes_path_as_name() {
        let dir = Path::new(".local/test");
        std::fs::create_dir_all(dir).unwrap();
        let path = dir.join("test_name.txt");
        std::fs::write(&path, "aaa\n").unwrap();

        let doc = Document::from_file(&path).unwrap();
        assert_eq!(doc.name(), Some(path.display().to_string().as_str()));

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
