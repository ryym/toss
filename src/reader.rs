use std::marker::PhantomData;

use crate::{
    error::AnyError,
    source::{QueryBlock, Source, SourceCursor},
};

/// Byte length of a Line Break.
const BLB: u64 = 1;

/// A range of the line in the source text.
/// The `end_byte` is exclusive and doesn't contain a line break.
/// ```text
/// abc\n  : start=0, end=3
/// defg\n : start=4, end=8
/// hi     : start=9, end=11
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct LinePos {
    start_byte: u64,
    end_byte: u64,
}

pub(crate) type Line = (LinePos, String);

impl LinePos {
    fn new(start_byte: u64, end_byte: u64) -> Self {
        Self {
            start_byte,
            end_byte,
        }
    }

    #[inline]
    pub fn start_byte(&self) -> u64 {
        self.start_byte
    }

    #[inline]
    pub fn end_byte(&self) -> u64 {
        self.end_byte
    }

    pub fn next_start_byte(&self) -> u64 {
        self.end_byte + BLB
    }

    pub fn prev_end_byte(&self) -> Option<u64> {
        if self.start_byte == 0 {
            None
        } else {
            Some(self.start_byte - BLB)
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum QueryLine {
    AtStart,
    AtEnd,
    At(LinePos),
    NextOf(LinePos),
    PrevOf(LinePos),
}

#[derive(Debug)]
enum SourceEnd {
    Unknown,
    LineBreak(u64),
    NonLineBreak,
}

/// Reader reads text of the given source line by line.
#[derive(Debug)]
pub(crate) struct Reader<R, Src> {
    _phantom: PhantomData<R>,
    source: Src,
    source_end: SourceEnd,
}

impl<R, Src: Source<R>> Reader<R, Src> {
    pub(crate) fn new(source: Src) -> Self {
        Self {
            _phantom: PhantomData,
            source,
            source_end: SourceEnd::Unknown,
        }
    }

    pub(crate) fn lines_from(&mut self, query: QueryLine) -> LineCursor<'_, R, Src> {
        LineCursor::new(self, query, true)
    }

    pub(crate) fn lines_rev_from(&mut self, query: QueryLine) -> LineCursor<'_, R, Src> {
        LineCursor::new(self, query, false)
    }

    pub(crate) fn read_line(&mut self, query: QueryLine) -> Result<Option<Line>, AnyError> {
        match query {
            QueryLine::AtStart => self.read_line_forward(0),
            QueryLine::AtEnd => self.read_end_line(),
            QueryLine::At(pos) => self.read_line_forward(pos.start_byte),
            QueryLine::NextOf(pos) => self.read_line_forward(pos.end_byte + BLB),
            QueryLine::PrevOf(pos) => {
                if pos.start_byte == 0 {
                    Ok(None)
                } else {
                    self.read_line_ending_at(pos.start_byte - BLB)
                }
            }
        }
    }

    fn read_line_forward(&mut self, start_byte: u64) -> Result<Option<Line>, AnyError> {
        let mut cursor = SourceCursor::forward(&mut self.source, QueryBlock::Having(start_byte));
        if !cursor.has_next()? {
            return Ok(None);
        }

        let mut buf = Vec::new();
        'outer: while let Some(content) = cursor.next_content()? {
            for b in content {
                if *b == b'\n' {
                    break 'outer;
                }
                buf.push(*b);
            }
        }

        let pos = LinePos::new(start_byte, start_byte + buf.len() as u64);
        let text = String::from_utf8_lossy(&buf).to_string();
        Ok(Some((pos, text)))
    }

    fn read_line_ending_at(&mut self, line_break_byte: u64) -> Result<Option<Line>, AnyError> {
        if line_break_byte == 0 {
            return Ok(None);
        }
        let line_end_byte = line_break_byte - 1;
        self.read_line_backward(QueryBlock::Having(line_end_byte))
    }

    fn read_end_line(&mut self) -> Result<Option<Line>, AnyError> {
        match self.source_end {
            SourceEnd::NonLineBreak => self.read_line_backward(QueryBlock::AtEnd),
            SourceEnd::LineBreak(line_end_byte) => {
                self.read_line_backward(QueryBlock::Having(line_end_byte))
            }
            SourceEnd::Unknown => {
                self.source_end = match self.read_line_backward(QueryBlock::AtEnd)? {
                    Some((pos, text)) if text.is_empty() => {
                        SourceEnd::LineBreak(pos.end_byte - BLB - 1)
                    }
                    _ => SourceEnd::NonLineBreak,
                };
                self.read_end_line()
            }
        }
    }

    fn read_line_backward(&mut self, from: QueryBlock) -> Result<Option<Line>, AnyError> {
        let mut cursor = SourceCursor::backward(&mut self.source, from);
        let end_byte = match cursor.cursor_pos()? {
            None => return Ok(None),
            Some(byte) => byte + 1,
        };
        let mut buf = Vec::new();
        'outer: while let Some(content) = cursor.next_content()? {
            for b in content.iter().rev() {
                if *b == b'\n' {
                    break 'outer;
                }
                buf.push(*b);
            }
        }
        buf.reverse();

        let pos = LinePos::new(end_byte - buf.len() as u64, end_byte);
        let text = String::from_utf8_lossy(&buf).to_string();
        Ok(Some((pos, text)))
    }
}

/// LineCursor provides an iterator-like interface to read lines one by one.
pub(crate) struct LineCursor<'r, R, Src: Source<R>> {
    reader: &'r mut Reader<R, Src>,
    query: QueryLine,
    go_forward: bool,
}

impl<'r, R, Src: Source<R>> LineCursor<'r, R, Src> {
    fn new(reader: &'r mut Reader<R, Src>, query: QueryLine, go_forward: bool) -> Self {
        Self {
            reader,
            query,
            go_forward,
        }
    }

    pub fn next(&mut self) -> Result<Option<Line>, AnyError> {
        match self.reader.read_line(self.query)? {
            None => Ok(None),
            Some((pos, text)) => {
                self.query = if self.go_forward {
                    QueryLine::NextOf(pos)
                } else {
                    QueryLine::PrevOf(pos)
                };
                Ok(Some((pos, text)))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use pretty_assertions::assert_eq;

    use crate::{
        error::AnyError,
        reader::{LinePos, QueryLine, Reader},
        source::Source,
    };

    #[test]
    fn readable_read_lines_back_and_forth() -> Result<(), AnyError> {
        test_readable_read_lines_back_and_forth(crate::source::as_readable)
    }
    #[test]
    fn seekable_read_lines_back_and_forth() -> Result<(), AnyError> {
        test_readable_read_lines_back_and_forth(crate::source::as_seekable)
    }
    fn test_readable_read_lines_back_and_forth<S, F>(make_source: F) -> Result<(), AnyError>
    where
        S: Source<Cursor<String>>,
        F: Fn(Cursor<String>) -> S,
    {
        let s = "abcde\n1234567\n890".to_string();
        let cursor = Cursor::new(s);
        let source = make_source(cursor);
        let mut reader = Reader::new(source);

        // Read forward
        {
            let (pos, text) = reader.read_line(QueryLine::AtStart)?.unwrap();
            assert_eq!(text, "abcde");
            assert_eq!(pos, LinePos::new(0, 5));

            let (pos, text) = reader.read_line(QueryLine::NextOf(pos))?.unwrap();
            assert_eq!(text, "1234567");
            assert_eq!(pos, LinePos::new(6, 13));

            let (pos, text) = reader.read_line(QueryLine::NextOf(pos))?.unwrap();
            assert_eq!(text, "890");
            assert_eq!(pos, LinePos::new(14, 17));

            let result = reader.read_line(QueryLine::NextOf(pos))?;
            assert_eq!(result, None);

            let (_pos, text) = reader.read_line(QueryLine::AtStart)?.unwrap();
            assert_eq!(text, "abcde");
        }

        // Read backward
        {
            let (pos, text) = reader.read_line(QueryLine::AtEnd)?.unwrap();
            assert_eq!(text, "890");
            assert_eq!(pos, LinePos::new(14, 17));

            let (pos, text) = reader.read_line(QueryLine::PrevOf(pos))?.unwrap();
            assert_eq!(text, "1234567");
            assert_eq!(pos, LinePos::new(6, 13));

            let (pos, text) = reader.read_line(QueryLine::PrevOf(pos))?.unwrap();
            assert_eq!(text, "abcde");
            assert_eq!(pos, LinePos::new(0, 5));

            let result = reader.read_line(QueryLine::PrevOf(pos))?;
            assert_eq!(result, None);
        }

        Ok(())
    }
}
