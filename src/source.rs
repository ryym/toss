use std::{
    collections::{HashMap, hash_map::Entry},
    io::{BufReader, Read, Seek, SeekFrom},
};

use crate::{
    AppResult,
    source::block::{BLOCK_SIZE, Block, BlockKey, read_block_from},
};

mod block;
mod cursor;

pub(crate) use cursor::SourceCursor;

type BlockPool = HashMap<BlockKey, Block>;

/// Source is a source text and provide a convenient way of reading content on demand.
/// It caches loaded content per "block" with a fixed byte size. Disk accesses occur sparsely.
pub(crate) trait Source<R> {
    fn read_block(&mut self, query: QueryBlock) -> AppResult<Option<&Block>>;
}

pub(crate) fn as_readable<R: Read>(reader: R) -> impl Source<R> {
    OneDirectionalSource::new(reader)
}

pub(crate) fn as_seekable<R: Read + Seek>(reader: R) -> impl Source<R> {
    SeekableSource::new(reader)
}

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

    fn read_and_cache_block(&mut self, byte_index: u64) -> AppResult<Option<&Block>> {
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
    fn read_block(&mut self, query: QueryBlock) -> AppResult<Option<&Block>> {
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

    fn read_and_cache_block(&mut self, byte_index: u64) -> AppResult<Option<&Block>> {
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
    fn read_block(&mut self, query: QueryBlock) -> AppResult<Option<&Block>> {
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

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use pretty_assertions::assert_eq;

    use crate::{
        AppResult,
        source::{QueryBlock, Source},
    };

    #[test]
    fn read_as_readable() -> AppResult<()> {
        test_read_source(crate::source::as_readable)
    }
    #[test]
    fn read_as_seekable() -> AppResult<()> {
        test_read_source(crate::source::as_seekable)
    }
    fn test_read_source<S, F>(make_source: F) -> AppResult<()>
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
