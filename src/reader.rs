use std::marker::PhantomData;

use crate::{
    error::AnyError,
    source::{BlockPos, Source},
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct LinePos {
    start_byte: u64,
    end_byte: u64,
}

impl LinePos {
    fn new(start_byte: u64, end_byte: u64) -> Self {
        Self {
            start_byte,
            end_byte,
        }
    }
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

    pub(crate) fn read_next_line(
        &mut self,
        prev: Option<LinePos>,
        s: &mut String,
    ) -> Result<Option<LinePos>, AnyError> {
        let start_byte = prev.map(|p| p.end_byte + 1).unwrap_or(0);

        let mut block_pos = BlockPos::At(start_byte);
        if self.source.read_block(block_pos)?.is_none() {
            return Ok(None);
        }

        s.clear();
        let buf = unsafe { s.as_mut_vec() };
        let mut block_start_byte = start_byte;
        'outer: while let Some(block) = self.source.read_block(block_pos)? {
            let content = block.slice_from(block_start_byte);
            for b in content {
                if *b == b'\n' {
                    break 'outer;
                }
                buf.push(*b);
            }
            block_start_byte = block.next_block_start_byte();
            block_pos = BlockPos::At(block_start_byte);
        }

        Ok(Some(LinePos::new(
            start_byte,
            start_byte + buf.len() as u64,
        )))
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use pretty_assertions::assert_eq;

    use crate::{
        error::AnyError,
        reader::{LinePos, Reader},
        source::Source,
    };

    fn test_readable_read_lines_forward<S, F>(make_source: F) -> Result<(), AnyError>
    where
        S: Source<Cursor<String>>,
        F: Fn(Cursor<String>) -> S,
    {
        let s = "abcde\n1234567".to_string();
        let cursor = Cursor::new(s);
        let source = make_source(cursor);
        let mut reader = Reader::new(source);

        let mut out = String::new();
        let pos = reader.read_next_line(None, &mut out)?;
        assert_eq!(out, "abcde");
        assert_eq!(pos, Some(LinePos::new(0, 5)));

        let pos = reader.read_next_line(pos, &mut out)?;
        assert_eq!(out, "1234567");
        assert_eq!(pos, Some(LinePos::new(6, 13)));

        let pos = reader.read_next_line(pos, &mut out)?;
        assert_eq!(out, "1234567");
        assert_eq!(pos, None);

        Ok(())
    }
    #[test]
    fn readable_read_lines_forward() -> Result<(), AnyError> {
        test_readable_read_lines_forward(crate::source::as_readable)
    }
    #[test]
    fn seekable_read_lines_forward() -> Result<(), AnyError> {
        test_readable_read_lines_forward(crate::source::as_seekable)
    }
}
