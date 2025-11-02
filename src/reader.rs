use std::marker::PhantomData;

use crate::{
    error::AnyError,
    source::{QueryBlock, Source, SourceCursor},
};

// XXX: 空ファイルの場合は結局どうなるんだっけ？
// その場合も read_line は空行を返せる方がいい？　今は None?

/// Byte length of a Line Break.
const BLB: u64 = 1;

/// A range of the line in the source text.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct LinePos {
    start_byte: u64,
    end_byte: u64,
}

pub(crate) type Line = (LinePos, String);

// #[derive(Debug, PartialEq)]
// pub(crate) struct Line {
//     pos: LinePos,
//     text: String,
// }
// impl Line {
//     fn new(pos: LinePos, text: String) -> Self {
//         Self { pos, text }
//     }
// }

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

    // #[cfg(test)]
    // pub fn mock(start_byte: u64, end_byte: u64) -> Self {
    //     Self::new(start_byte, end_byte)
    // }
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
enum SourceEnd {
    Unknown,
    LineBreak(u64),
    NonLineBreak,
}

#[derive(Debug)]
pub(crate) struct Reader<R, S> {
    _phantom: PhantomData<R>,
    source: S,
    source_end: SourceEnd,
}

impl<R, S: Source<R>> Reader<R, S> {
    pub(crate) fn new(source: S) -> Self {
        Self {
            _phantom: PhantomData,
            source,
            source_end: SourceEnd::Unknown,
        }
    }

    // XXX: 各位置の行を読めるのは必要だと思うが、
    // このインターフェイスが本当に最適なのかは要検討。
    // 普通に個別にメソッドを提供するだけの方がシンプルかもしれない。
    // read_block は trait method だからあまり増やさない方が楽だったけど。
    // あと String を受け取るかどうか。
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
                dbg!(&self.source_end);
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
