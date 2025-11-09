use std::collections::{HashMap, hash_map::Entry};

use crate::{
    error::AnyError,
    pager::{PageLine, PageSize},
    reader::{LinePos, QueryLine, Reader},
    source::Source,
};

#[derive(Debug)]
pub(super) struct LineMeta {
    pos: LinePos,
}

impl LineMeta {
    #[inline]
    pub fn pos(&self) -> LinePos {
        self.pos
    }
}

pub(super) type Line = PageLine<LineMeta>;

/// LineStore reads lines from the given [`Reader`] and caches them.
#[derive(Debug)]
pub(super) struct LineStore<R, Src> {
    reader: Reader<R, Src>,
    line_map: HashMap<u64, Line>,
    end_to_start: HashMap<u64, u64>,
    end_line_start_byte: Option<u64>,
    col_size: usize,
}

impl<R, Src: Source<R>> LineStore<R, Src> {
    pub fn new(reader: Reader<R, Src>, size: &PageSize) -> Self {
        let capacity = size.rows() * 2;
        Self {
            reader,
            line_map: HashMap::with_capacity(capacity),
            end_to_start: HashMap::with_capacity(capacity),
            end_line_start_byte: None,
            col_size: size.cols(),
        }
    }

    pub fn read_line(&mut self, query: &QueryLine) -> Result<Option<&mut Line>, AnyError> {
        self.free_line_space_if_needed();
        match query {
            QueryLine::AtStart => self.read_line_by_start_byte(0, query),
            QueryLine::AtEnd => self.read_end_line(),
            QueryLine::At(pos) => self.read_line_by_end_byte(pos.start_byte(), query),
            QueryLine::NextOf(pos) => self.read_line_by_start_byte(pos.next_start_byte(), query),
            QueryLine::PrevOf(pos) => {
                let prev_end_byte = match pos.prev_end_byte() {
                    None => return Ok(None),
                    Some(byte) => byte,
                };
                self.read_line_by_end_byte(prev_end_byte, query)
            }
        }
    }

    fn free_line_space_if_needed(&mut self) {
        if self.line_map.len() < self.line_map.capacity() - 1 {
            return;
        }
        // Simply clear all cached data for now.
        self.line_map.clear();
        self.end_to_start.clear();
    }

    fn read_line_by_start_byte(
        &mut self,
        start_byte: u64,
        query: &QueryLine,
    ) -> Result<Option<&mut Line>, AnyError> {
        match self.line_map.entry(start_byte) {
            Entry::Occupied(entry) => Ok(Some(entry.into_mut())),
            Entry::Vacant(entry) => match self.reader.read_line(query)? {
                None => Ok(None),
                Some((pos, text)) => {
                    let line = Line::new(LineMeta { pos }, text, self.col_size);
                    self.end_to_start.insert(pos.end_byte(), pos.start_byte());
                    Ok(Some(entry.insert(line)))
                }
            },
        }
    }

    fn read_line_by_end_byte(
        &mut self,
        end_byte: u64,
        query: &QueryLine,
    ) -> Result<Option<&mut Line>, AnyError> {
        let start_byte = match self.end_to_start.entry(end_byte) {
            Entry::Occupied(entry) => *entry.get(),
            Entry::Vacant(entry) => match self.reader.read_line(query)? {
                None => return Ok(None),
                Some((pos, text)) => {
                    let line = Line::new(LineMeta { pos }, text, self.col_size);
                    entry.insert(pos.start_byte());
                    self.line_map.insert(pos.start_byte(), line);
                    self.end_to_start.insert(pos.end_byte(), pos.start_byte());
                    pos.start_byte()
                }
            },
        };
        Ok(self.line_map.get_mut(&start_byte))
    }

    fn read_end_line(&mut self) -> Result<Option<&mut Line>, AnyError> {
        match self.end_line_start_byte {
            Some(start_byte) => self.read_line_by_start_byte(start_byte, &QueryLine::AtEnd),
            None => match self.reader.read_line(&QueryLine::AtEnd)? {
                None => Ok(None),
                Some((pos, text)) => {
                    let line = Line::new(LineMeta { pos }, text, self.col_size);
                    self.end_line_start_byte = Some(pos.start_byte());
                    self.line_map.insert(pos.start_byte(), line);
                    self.end_to_start.insert(pos.end_byte(), pos.start_byte());
                    Ok(self.line_map.get_mut(&pos.start_byte()))
                }
            },
        }
    }
}
