use std::{
    cmp,
    collections::HashMap,
    io::{BufReader, Read},
};

use crate::error::AnyError;

pub(super) const BLOCK_SIZE: u64 = 8192;

type BlockBuffer = [u8; BLOCK_SIZE as usize];

/// Block is a cache unit of the source text.
#[derive(Debug)]
pub(crate) struct Block {
    start_byte: u64,
    end_byte: u64,
    buffer: BlockBuffer,
}

impl Block {
    pub(super) fn new(end_byte: u64, buffer: BlockBuffer) -> Self {
        Self {
            start_byte: BLOCK_SIZE * ((end_byte - 1) / BLOCK_SIZE),
            end_byte,
            buffer,
        }
    }

    #[inline]
    pub(super) fn start_byte(&self) -> u64 {
        self.start_byte
    }

    #[inline]
    pub(super) fn end_byte(&self) -> u64 {
        self.end_byte
    }

    pub(super) fn slice_from(&self, abs_byte: u64) -> &[u8] {
        let from = (abs_byte - self.start_byte) as usize;
        let to = (self.end_byte - self.start_byte) as usize;
        &self.buffer[from..to]
    }

    pub(super) fn slice_to(&self, abs_byte: u64) -> &[u8] {
        let end_byte = cmp::min(abs_byte, self.end_byte);
        let to = (end_byte - self.start_byte) as usize;
        &self.buffer[..to]
    }

    pub(super) fn content(&self) -> &[u8] {
        let remainder = self.end_byte % BLOCK_SIZE;
        if remainder == 0 {
            &self.buffer
        } else {
            &self.buffer[..(remainder as usize)]
        }
    }
}

/// A key of block pool.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct BlockKey {
    start_byte: u64,
}

impl BlockKey {
    pub(super) fn from_byte(byte_index: u64) -> Self {
        let start_byte = BLOCK_SIZE * (byte_index / BLOCK_SIZE);
        Self { start_byte }
    }

    #[inline]
    pub(super) fn start_byte(&self) -> u64 {
        self.start_byte
    }
}

// XXX: これここ？
pub(super) type BlockPool = HashMap<BlockKey, Block>;

pub(super) fn read_block_from<R: Read>(
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
