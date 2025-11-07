use std::collections::{HashMap, hash_map::Entry};

use crate::{
    error::AnyError,
    pager::PageSize,
    reader::{LinePos, QueryLine, Reader},
    source::Source,
};

// XXX
pub(super) type PageLine = crate::pager::PageLine<LinePos>;

#[derive(Debug)]
pub(super) struct LineReader<R, Src> {
    reader: Reader<R, Src>,
    line_cache: HashMap<u64, PageLine>,
    end_to_start: HashMap<u64, u64>,
    end_line_start_byte: Option<u64>,
    col_size: usize,
}

impl<R, Src: Source<R>> LineReader<R, Src> {
    pub fn new(reader: Reader<R, Src>, size: &PageSize) -> Self {
        Self {
            reader,
            // xxx: default capacity
            line_cache: HashMap::with_capacity(size.rows()),
            end_to_start: HashMap::with_capacity(size.rows()),
            end_line_start_byte: None,
            col_size: size.cols(),
        }
    }

    pub fn read_line(&mut self, query: QueryLine) -> Result<Option<&mut PageLine>, AnyError> {
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

    fn read_line_by_start_byte(
        &mut self,
        start_byte: u64,
        query: QueryLine,
    ) -> Result<Option<&mut PageLine>, AnyError> {
        // match self.line_cache.get_mut(&start_byte) {
        //     Some(line) => Ok(Some(line)),
        //     None => match self.reader.read_line(query)? {
        //         None => Ok(None),
        //         Some((pos, text)) => {
        //             let line = PageLine::new(pos, text, self.col_size);
        //             self.end_to_start.insert(pos.end_byte(), pos.start_byte());
        //             // Ok(Some(entry.insert(line)))

        //             // XXX: entry なしだと insert して更に get_mut?

        //             todo!()
        //         }
        //     },
        // }
        match self.line_cache.entry(start_byte) {
            Entry::Occupied(entry) => Ok(Some(entry.into_mut())),
            Entry::Vacant(entry) => match self.reader.read_line(query)? {
                None => Ok(None),
                Some((pos, text)) => {
                    let line = PageLine::new(pos, text, self.col_size);
                    self.end_to_start.insert(pos.end_byte(), pos.start_byte());
                    Ok(Some(entry.insert(line)))
                }
            },
        }
    }

    fn read_line_by_end_byte(
        &mut self,
        end_byte: u64,
        query: QueryLine,
    ) -> Result<Option<&mut PageLine>, AnyError> {
        let start_byte = match self.end_to_start.entry(end_byte) {
            Entry::Occupied(entry) => *entry.get(),
            Entry::Vacant(entry) => match self.reader.read_line(query)? {
                None => return Ok(None),
                Some((pos, text)) => {
                    let line = PageLine::new(pos, text, self.col_size);
                    entry.insert(pos.start_byte());
                    self.line_cache.insert(pos.start_byte(), line);
                    self.end_to_start.insert(pos.end_byte(), pos.start_byte());
                    pos.start_byte()
                }
            },
        };
        Ok(self.line_cache.get_mut(&start_byte))
    }

    fn read_end_line(&mut self) -> Result<Option<&mut PageLine>, AnyError> {
        let start_byte = match self.end_line_start_byte {
            Some(start_byte) => start_byte,
            None => match self.reader.read_line(QueryLine::AtEnd)? {
                None => return Ok(None),
                Some((pos, text)) => {
                    let line = PageLine::new(pos, text, self.col_size);
                    self.end_line_start_byte = Some(pos.start_byte());
                    self.line_cache.insert(pos.start_byte(), line);
                    self.end_to_start.insert(pos.end_byte(), pos.start_byte());
                    pos.start_byte()
                }
            },
        };
        Ok(self.line_cache.get_mut(&start_byte))
    }

    // fn remove_cache(&mut self, key: &LineKey) {
    //     let start_byte = match key {
    //         LineKey::StartByte(byte) => byte,
    //         LineKey::EndByte(byte) => &self.end_to_start[&byte],
    //     };
    //     if let Some(line) = self.line_cache.remove(start_byte) {
    //         self.end_to_start.remove(&line.meta().end_byte());
    //     }
    // }
}
