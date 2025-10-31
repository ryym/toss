use std::marker::PhantomData;

use crate::{
    error::AnyError,
    source::{QueryBlock, Source, SourceCursor},
};

/// Byte length of a Line Break.
const BLB: u64 = 1;

/// A range of the line in the source text.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct LinePos {
    start_byte: u64,
    end_byte: u64,
}

type LineAndPos = (LinePos, String);

impl LinePos {
    /// The `end_byte` is exclusive and doesn't contain a line break.
    /// ```text
    /// abc\n  : start=0, end=3
    /// defg\n : start=4, end=8
    /// hi     : start=9, end=11
    /// ```
    fn new(start_byte: u64, end_byte: u64) -> Self {
        Self {
            start_byte,
            end_byte,
        }
    }
}

#[derive(Debug)]
pub(crate) enum QueryLine {
    AtStart,
    AtEnd,
    At(LinePos),
    NextOf(LinePos),
    PrevOf(LinePos),
}

#[derive(Debug)]
pub(crate) struct Reader<R, S> {
    _phantom: PhantomData<R>,
    source: S,
}

impl<R, S: Source<R>> Reader<R, S> {
    pub(crate) fn new(source: S) -> Self {
        Self {
            _phantom: PhantomData,
            source,
        }
    }

    // XXX: 各位置の行を読めるのは必要だと思うが、
    // このインターフェイスが本当に最適なのかは要検討。
    // 普通に個別にメソッドを提供するだけの方がシンプルかもしれない。
    // read_block は trait method だからあまり増やさない方が楽だったけど。
    // あと String を受け取るかどうか。
    pub(crate) fn read_line(&mut self, query: QueryLine) -> Result<Option<LineAndPos>, AnyError> {
        match query {
            QueryLine::AtStart => self.read_line_forward(0),
            QueryLine::AtEnd => self.read_line_backward(QueryBlock::AtEnd),
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

    fn read_line_forward(&mut self, start_byte: u64) -> Result<Option<LineAndPos>, AnyError> {
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

        let s = String::from_utf8_lossy(&buf).to_string();
        let pos = LinePos::new(start_byte, start_byte + buf.len() as u64);
        Ok(Some((pos, s)))
    }

    fn read_line_ending_at(
        &mut self,
        line_break_byte: u64,
    ) -> Result<Option<LineAndPos>, AnyError> {
        if line_break_byte == 0 {
            return Ok(None);
        }
        let line_end_byte = line_break_byte - 1;
        self.read_line_backward(QueryBlock::Having(line_end_byte))
    }

    fn read_line_backward(&mut self, from: QueryBlock) -> Result<Option<LineAndPos>, AnyError> {
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

        let s = String::from_utf8_lossy(&buf).to_string();
        let pos = LinePos::new(end_byte - buf.len() as u64, end_byte);
        Ok(Some((pos, s)))
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
            let result = reader.read_line(QueryLine::AtStart)?;
            let (pos, out) = result.unwrap();
            assert_eq!(out, "abcde");
            assert_eq!(pos, LinePos::new(0, 5));

            let result = reader.read_line(QueryLine::NextOf(pos))?;
            let (pos, out) = result.unwrap();
            assert_eq!(out, "1234567");
            assert_eq!(pos, LinePos::new(6, 13));

            let result = reader.read_line(QueryLine::NextOf(pos))?;
            let (pos, out) = result.unwrap();
            assert_eq!(out, "890");
            assert_eq!(pos, LinePos::new(14, 17));

            let result = reader.read_line(QueryLine::NextOf(pos))?;
            assert_eq!(result, None);

            let result = reader.read_line(QueryLine::AtStart)?;
            let (_, out) = result.unwrap();
            assert_eq!(out, "abcde");
        }

        // Read backward
        {
            let result = reader.read_line(QueryLine::AtEnd)?;
            let (pos, out) = result.unwrap();
            assert_eq!(out, "890");
            assert_eq!(pos, LinePos::new(14, 17));

            let result = reader.read_line(QueryLine::PrevOf(pos))?;
            let (pos, out) = result.unwrap();
            assert_eq!(out, "1234567");
            assert_eq!(pos, LinePos::new(6, 13));

            let result = reader.read_line(QueryLine::PrevOf(pos))?;
            let (pos, out) = result.unwrap();
            assert_eq!(out, "abcde");
            assert_eq!(pos, LinePos::new(0, 5));

            let result = reader.read_line(QueryLine::PrevOf(pos))?;
            assert_eq!(result, None);
        }

        Ok(())
    }
}
