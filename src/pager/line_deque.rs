// use std::collections::VecDeque;

// use crate::{pager::line::Line, reader::LinePos};

// #[derive(Debug)]
// struct Row {
//     line_pos: LinePos,
//     deque_index: usize,
//     slice_index: usize,
// }

// // サイズ知ってないと駄目？

// pub(super) struct LineDeque {
//     deque: VecDeque<Line>,
//     start_row: Row,
//     end_row: Row,
// }

// pub(super) struct LineDequeBuilder {
//     deque: VecDeque<Line>,
//     end_row: Option<Row>,
// }

// impl LineDequeBuilder {
//     pub fn push_back(&mut self, line: Line) {
//         self.deque.push_back(line);
//     }

//     pub fn build(self) -> LineDeque {
//         assert!(!self.deque.is_empty());
//         let start_row = Row {
//             line_pos: self.deque[0].pos(),
//             deque_index: 0,
//             slice_index: 0,
//         };
//         let last = self.deque[self.deque.len() - 1];
//         let end_row = Row {
//             line_pos: last.pos(),
//             deque_index: self.deque.len() - 1,
//             slice_index: 0, // ?
//         };
//     }
// }
