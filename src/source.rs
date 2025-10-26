use std::{
    collections::{hash_map::Entry, HashMap},
    io::{BufReader, Read, Seek, SeekFrom},
};

use crate::error::AnyError;

pub(crate) fn as_readable<R: Read>(reader: R) -> impl Source<R> {
    OneDirectionalSource::new(reader)
}

pub(crate) fn as_seekable<R: Read + Seek>(reader: R) -> impl Source<R> {
    SeekableSource::new(reader)
}

const BLOCK_SIZE: u64 = 8192;

type BlockBuffer = [u8; BLOCK_SIZE as usize];

#[derive(Debug)]
pub(crate) struct Block {
    start_byte: u64,
    end_byte: u64,
    buffer: BlockBuffer,
}

impl Block {
    fn new(end_byte: u64, buffer: BlockBuffer) -> Self {
        Self {
            start_byte: BLOCK_SIZE * (end_byte / BLOCK_SIZE),
            end_byte,
            buffer,
        }
    }

    pub(crate) fn next_block_start_byte(&self) -> u64 {
        self.start_byte + BLOCK_SIZE
    }

    pub(crate) fn slice_from(&self, abs_byte: u64) -> &[u8] {
        let from = (abs_byte - self.start_byte) as usize;
        let to = (self.end_byte - self.start_byte) as usize;
        &self.buffer[from..to]
    }

    pub(crate) fn content(&self) -> &[u8] {
        let remainder = self.end_byte % BLOCK_SIZE;
        if remainder == 0 {
            &self.buffer
        } else {
            &self.buffer[..(remainder as usize)]
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct BlockKey {
    start_byte: u64,
}

impl BlockKey {
    pub(crate) fn from_byte(byte_index: u64) -> Self {
        let start_byte = BLOCK_SIZE * (byte_index / BLOCK_SIZE);
        Self { start_byte }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum BlockPos {
    At(u64),
    End,
}

pub(crate) trait Source<R> {
    fn read_block(&mut self, pos: BlockPos) -> Result<Option<&Block>, AnyError>;
}

type BlockPool = HashMap<BlockKey, Block>;

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
                let seek_pos = key.start_byte as i64 - self.seek_cursor;
                self.reader.seek_relative(seek_pos)?;
                match read_block_from(&mut self.reader, key.start_byte)? {
                    Some(block) => {
                        let block = entry.insert(block);
                        self.seek_cursor = block.end_byte as i64;
                        block
                    }
                    None => return Ok(None),
                }
            }
        };
        if byte_index <= block.end_byte {
            Ok(Some(block))
        } else {
            Ok(None)
        }
    }
}

impl<R: Read + Seek> Source<R> for SeekableSource<R> {
    fn read_block(&mut self, pos: BlockPos) -> Result<Option<&Block>, AnyError> {
        let block = match pos {
            BlockPos::At(byte_index) => self.read_and_cache_block(byte_index)?,
            BlockPos::End => {
                let total_byte = self.reader.seek(SeekFrom::End(0))?;
                self.seek_cursor = total_byte as i64;
                let last_block_start = BlockKey::from_byte(total_byte).start_byte;
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
            Entry::Vacant(entry) => match read_block_from(&mut self.reader, key.start_byte)? {
                Some(block) => entry.insert(block),
                None => return Ok(None),
            },
        };
        if byte_index <= block.end_byte {
            Ok(Some(block))
        } else {
            Ok(None)
        }
    }
}

impl<R: Read> Source<R> for OneDirectionalSource<R> {
    fn read_block(&mut self, from: BlockPos) -> Result<Option<&Block>, AnyError> {
        match from {
            BlockPos::At(byte_index) => {
                let mut byte_cursor = 0;
                while let Some(block) = self.read_and_cache_block(byte_cursor)? {
                    if block.start_byte <= byte_index {
                        break;
                    }
                    byte_cursor += BLOCK_SIZE;
                }
                self.read_and_cache_block(byte_index)
            }
            BlockPos::End => {
                let mut block_start = 0;
                while self.read_and_cache_block(block_start)?.is_some() {
                    block_start += BLOCK_SIZE;
                }
                let block = self.read_and_cache_block(block_start)?;
                Ok(block)
            }
        }
    }
}

fn read_block_from<R: Read>(
    reader: &mut BufReader<R>,
    start_byte: u64,
) -> Result<Option<Block>, AnyError> {
    let mut buf: [u8; BLOCK_SIZE as usize] = [0; BLOCK_SIZE as usize];
    let mut bytes_read = 0;
    while bytes_read < BLOCK_SIZE as usize {
        match reader.read(&mut buf[bytes_read..])? {
            0 => break,
            n => bytes_read += n,
        }
    }
    if bytes_read > 0 {
        Ok(Some(Block::new(start_byte + bytes_read as u64, buf)))
    } else {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use pretty_assertions::assert_eq;

    use crate::{
        error::AnyError,
        source::{BlockPos, Source},
    };

    fn test_read_source<S, F>(make_source: F) -> Result<(), AnyError>
    where
        S: Source<Cursor<String>>,
        F: Fn(Cursor<String>) -> S,
    {
        let s = "abcde".to_string();
        let cursor = Cursor::new(s);
        let mut source = make_source(cursor);

        let block = source.read_block(BlockPos::At(0))?;
        assert!(block.is_some());

        let block = block.unwrap();
        assert_eq!(block.slice_from(0), b"abcde");
        assert_eq!(block.slice_from(2), b"cde");

        Ok(())
    }
    #[test]
    fn read_as_readable() -> Result<(), AnyError> {
        test_read_source(crate::source::as_readable)
    }
    #[test]
    fn read_as_seekable() -> Result<(), AnyError> {
        test_read_source(crate::source::as_seekable)
    }
}
