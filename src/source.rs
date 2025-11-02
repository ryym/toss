use std::{
    cmp,
    collections::hash_map::Entry,
    io::{BufReader, Read, Seek, SeekFrom},
    marker::PhantomData,
};

use crate::{
    error::AnyError,
    source::block::{BLOCK_SIZE, Block, BlockKey, BlockPool, read_block_from},
};

mod block;

pub(crate) fn as_readable<R: Read>(reader: R) -> impl Source<R> {
    OneDirectionalSource::new(reader)
}

pub(crate) fn as_seekable<R: Read + Seek>(reader: R) -> impl Source<R> {
    SeekableSource::new(reader)
}

/// A query to get a specific block.
#[derive(Debug, Clone, Copy)]
pub(crate) enum QueryBlock {
    Having(u64),
    AtEnd,
}

impl QueryBlock {
    fn next_of(block: &Block) -> Self {
        Self::Having(block.end_byte())
    }

    fn prev_of(block: &Block) -> Option<Self> {
        if block.start_byte() == 0 {
            None
        } else {
            Some(Self::Having(block.start_byte() - 1))
        }
    }

    fn byte_index(&self) -> Option<u64> {
        match self {
            Self::Having(byte) => Some(*byte),
            Self::AtEnd => None,
        }
    }
}

pub(crate) trait Source<R> {
    fn read_block(&mut self, query: QueryBlock) -> Result<Option<&Block>, AnyError>;
}

pub(crate) struct SeekableSource<R> {
    reader: BufReader<R>,
    pool: BlockPool,
    seek_cursor: i64,
    // todo: have a LRU linked list to limit the number of pooled blocks.
}

impl<R: Read + Seek> SeekableSource<R> {
    pub(crate) fn new(reader: R) -> Self {
        Self {
            reader: BufReader::new(reader),
            pool: BlockPool::new(),
            seek_cursor: 0,
        }
    }

    fn read_and_cache_block(&mut self, byte_index: u64) -> Result<Option<&Block>, AnyError> {
        let key = BlockKey::from_byte(byte_index);
        let entry = self.pool.entry(key);
        let block = match entry {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => {
                let seek_pos = key.start_byte() as i64 - self.seek_cursor;
                self.reader.seek_relative(seek_pos)?;
                match read_block_from(&mut self.reader, key.start_byte())? {
                    Some(block) => {
                        let block = entry.insert(block);
                        self.seek_cursor = block.end_byte() as i64;
                        block
                    }
                    None => return Ok(None),
                }
            }
        };
        if byte_index < block.end_byte() {
            Ok(Some(block))
        } else {
            Ok(None)
        }
    }
}

impl<R: Read + Seek> Source<R> for SeekableSource<R> {
    fn read_block(&mut self, query: QueryBlock) -> Result<Option<&Block>, AnyError> {
        let block = match query {
            QueryBlock::Having(byte_index) => self.read_and_cache_block(byte_index)?,
            QueryBlock::AtEnd => {
                let total_byte = self.reader.seek(SeekFrom::End(0))?;
                self.seek_cursor = total_byte as i64;
                let last_block_start = BlockKey::from_byte(total_byte).start_byte();
                self.read_and_cache_block(last_block_start)?
            }
        };
        Ok(block)
    }
}

pub(crate) struct OneDirectionalSource<R> {
    reader: BufReader<R>,
    pool: BlockPool,
}

impl<R: Read> OneDirectionalSource<R> {
    pub(crate) fn new(reader: R) -> Self {
        Self {
            reader: BufReader::new(reader),
            pool: BlockPool::new(),
        }
    }

    fn read_and_cache_block(&mut self, byte_index: u64) -> Result<Option<&Block>, AnyError> {
        let key = BlockKey::from_byte(byte_index);
        let entry = self.pool.entry(key);
        let block = match entry {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => match read_block_from(&mut self.reader, key.start_byte())? {
                Some(block) => entry.insert(block),
                None => return Ok(None),
            },
        };
        if byte_index < block.end_byte() {
            Ok(Some(block))
        } else {
            Ok(None)
        }
    }
}

impl<R: Read> Source<R> for OneDirectionalSource<R> {
    fn read_block(&mut self, query: QueryBlock) -> Result<Option<&Block>, AnyError> {
        match query {
            QueryBlock::Having(byte_index) => {
                let mut byte_cursor = 0;
                while let Some(block) = self.read_and_cache_block(byte_cursor)? {
                    if block.start_byte() <= byte_index {
                        break;
                    }
                    byte_cursor += BLOCK_SIZE;
                }
                self.read_and_cache_block(byte_index)
            }
            QueryBlock::AtEnd => {
                let mut block_start = 0;
                while self.read_and_cache_block(block_start)?.is_some() {
                    block_start += BLOCK_SIZE;
                }
                block_start -= BLOCK_SIZE;
                let block = self.read_and_cache_block(block_start)?;
                Ok(block)
            }
        }
    }
}

pub(crate) struct SourceCursor<'src, R, Src: Source<R>> {
    _phantom: PhantomData<R>,
    source: &'src mut Src,
    content_start_byte: Option<u64>,
    content_end_byte: Option<u64>,
    query: Option<QueryBlock>,
    go_forward: bool,
}

impl<'src, R, Src: Source<R>> SourceCursor<'src, R, Src> {
    pub fn forward(source: &'src mut Src, from: QueryBlock) -> Self {
        Self {
            _phantom: PhantomData,
            source,
            content_start_byte: from.byte_index(),
            content_end_byte: None,
            query: Some(from),
            go_forward: true,
        }
    }

    pub fn backward(source: &'src mut Src, from: QueryBlock) -> Self {
        Self {
            _phantom: PhantomData,
            source,
            content_start_byte: None,
            content_end_byte: from.byte_index().map(|b| b + 1), // exclusive
            query: Some(from),
            go_forward: false,
        }
    }

    pub fn cursor_pos(&mut self) -> Result<Option<u64>, AnyError> {
        let query = match self.query {
            None => return Ok(None),
            Some(query) => query,
        };
        let byte = match self.source.read_block(query)? {
            None => return Ok(None),
            Some(block) => match query {
                QueryBlock::Having(byte) => byte,
                QueryBlock::AtEnd => block.end_byte() - 1,
            },
        };
        Ok(Some(byte))
    }

    pub fn has_next(&mut self) -> Result<bool, AnyError> {
        Ok(self.cursor_pos()?.is_some())
    }

    pub fn next_content(&mut self) -> Result<Option<&[u8]>, AnyError> {
        if self.go_forward {
            self.read_forward()
        } else {
            self.read_backward()
        }
    }

    fn read_forward(&mut self) -> Result<Option<&[u8]>, AnyError> {
        let query = match self.query {
            None => return Ok(None),
            Some(query) => query,
        };
        match self.source.read_block(query)? {
            None => Ok(None),
            Some(block) => {
                self.query = Some(QueryBlock::next_of(block));
                let start_byte = cmp::max(block.start_byte(), self.content_start_byte.unwrap_or(0));
                Ok(Some(block.slice_from(start_byte)))
            }
        }
    }

    fn read_backward(&mut self) -> Result<Option<&[u8]>, AnyError> {
        let query = match self.query {
            None => return Ok(None),
            Some(query) => query,
        };
        match self.source.read_block(query)? {
            None => Ok(None),
            Some(block) => {
                self.query = QueryBlock::prev_of(block);
                if let Some(content_end_byte) = self.content_end_byte {
                    let end_byte = cmp::min(block.end_byte(), content_end_byte);
                    Ok(Some(block.slice_to(end_byte)))
                } else {
                    Ok(Some(block.content()))
                }
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
        source::{QueryBlock, Source},
    };

    #[test]
    fn read_as_readable() -> Result<(), AnyError> {
        test_read_source(crate::source::as_readable)
    }
    #[test]
    fn read_as_seekable() -> Result<(), AnyError> {
        test_read_source(crate::source::as_seekable)
    }
    fn test_read_source<S, F>(make_source: F) -> Result<(), AnyError>
    where
        S: Source<Cursor<String>>,
        F: Fn(Cursor<String>) -> S,
    {
        let s = "abcde".to_string();
        let cursor = Cursor::new(s);
        let mut source = make_source(cursor);

        let block = source.read_block(QueryBlock::Having(0))?;
        assert!(block.is_some());

        let block = block.unwrap();
        assert_eq!(block.slice_from(0), b"abcde");
        assert_eq!(block.slice_from(2), b"cde");

        Ok(())
    }
}
