use std::{cmp, marker::PhantomData};

use crate::{
    AppResult,
    source::{QueryBlock, Source},
};

/// SourceCursor provides an iterator-like interface to load content blocks one by one.
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

    pub fn cursor_pos(&mut self) -> AppResult<Option<u64>> {
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

    pub fn has_next(&mut self) -> AppResult<bool> {
        Ok(self.cursor_pos()?.is_some())
    }

    pub fn next_content(&mut self) -> AppResult<Option<&[u8]>> {
        if self.go_forward {
            self.read_forward()
        } else {
            self.read_backward()
        }
    }

    fn read_forward(&mut self) -> AppResult<Option<&[u8]>> {
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

    fn read_backward(&mut self) -> AppResult<Option<&[u8]>> {
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
