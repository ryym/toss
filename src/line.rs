use std::ops::Range;

use regex::Regex;

use crate::{AppResult, io::WriteStr, terminal};

/// A line in target text. It can be decorated with ANSI escape sequences.
#[derive(Debug, Default)]
pub(crate) struct Line {
    /// An original line including ANSI escape sequences.
    raw: String,

    /// A plain text line without escape sequences.
    plain: String,

    /// A byte index mapping from each character in `plain` to `raw`. Example:
    /// - raw: `\x1b[1mHi\x1b[0m, 😀` ("Hi" is bold)
    /// - plain: `Hi, 😀`
    /// - plain_to_raw:
    ///   ```text
    ///   (escape sequences: bold start)
    ///   0:  4 ──── H
    ///   1:  5 ──── i
    ///   (escape sequences: reset)
    ///   2: 10 ──── ,
    ///   3: 11 ──── (space)
    ///   4: 12 ──┬─ 😀
    ///   5: 13 ──┤
    ///   6: 14 ──┤
    ///   7: 15 ──┘
    ///   ```
    plain_to_raw: Vec<usize>,
}

impl Line {
    pub fn new(raw_line: String) -> Self {
        let mut plain = String::with_capacity(raw_line.len());
        let mut plain_to_raw = Vec::new();
        let mut i_raw = 0;
        for part in terminal::parse_text(&raw_line) {
            match part {
                terminal::Text::Control(s) => {
                    i_raw += s.len();
                }
                terminal::Text::Plain(s) => {
                    plain.push_str(s);
                    for _ in s.as_bytes() {
                        plain_to_raw.push(i_raw);
                        i_raw += 1;
                    }
                }
            }
        }
        Self {
            raw: raw_line,
            plain,
            plain_to_raw,
        }
    }

    #[inline]
    pub fn raw(&self) -> &str {
        &self.raw
    }

    #[inline]
    pub fn plain(&self) -> &str {
        &self.plain
    }

    #[inline]
    pub fn plain_to_raw(&self) -> &[usize] {
        &self.plain_to_raw
    }

    pub fn search(&self, query: &Regex) -> LineHighlight {
        let mut positions = Vec::new();
        for m in query.find_iter(&self.plain) {
            let i_raw = self.plain_to_raw[m.start()];
            positions.push(HightlightPos::start(i_raw));

            let mut prev_i_p = m.start();
            for i_p in m.start()..m.end() {
                let i_raw = self.plain_to_raw[i_p];
                let prev_i_raw = self.plain_to_raw[prev_i_p];
                // If there are more than one byte distances, it means there are escape sequences.
                if i_raw - prev_i_raw > 1 {
                    positions.push(HightlightPos::inner_control_end(i_raw));
                }
                prev_i_p = i_p;
            }

            let i_raw = self
                .plain_to_raw
                .get(m.end())
                .copied()
                .unwrap_or(self.raw.len());
            positions.push(HightlightPos::end(i_raw));
        }

        LineHighlight { positions }
    }

    pub fn write_to(&self, w: &mut impl WriteStr, args: WriteLineArgs) -> AppResult<()> {
        if self.raw.is_empty() {
            return Ok(());
        }
        assert!(args.range.start < self.raw.len());
        assert!(args.range.end <= self.raw.len());

        let mut i_pos_from = 0;
        let mut range_start_in_highlight = false;
        for (i, pos) in args.highlight.positions.iter().enumerate() {
            if args.range.start <= pos.index {
                i_pos_from = i;
                break;
            }
            range_start_in_highlight = !matches!(pos.kind, HightlightPosKind::End);
        }
        if range_start_in_highlight {
            w.write_all(termion::style::Invert.as_ref())?;
        }

        let mut i_prev = args.range.start;
        let mut range_end_in_highlight = false;
        for pos in args.highlight.positions.iter().skip(i_pos_from) {
            if args.range.end <= pos.index {
                break;
            }
            w.write_all(&self.raw[i_prev..pos.index])?;
            i_prev = pos.index;
            match pos.kind {
                HightlightPosKind::Start | HightlightPosKind::InnerControlEnd => {
                    w.write_all(termion::style::Invert.as_ref())?;
                    range_end_in_highlight = true;
                }
                HightlightPosKind::End => {
                    w.write_all(termion::style::NoInvert.as_ref())?;
                    range_end_in_highlight = false;
                }
            }
        }
        w.write_all(&self.raw[i_prev..args.range.end])?;
        if range_end_in_highlight {
            w.write_all(termion::style::NoInvert.as_ref())?;
        }

        Ok(())
    }
}

#[derive(Debug)]
pub(crate) struct WriteLineArgs<'a> {
    pub range: Range<usize>,
    pub highlight: &'a LineHighlight,
}

#[derive(Debug, Default)]
pub(crate) struct LineHighlight {
    /// A list of [`HightlightPos`].
    /// It guarantees:
    /// - The first element is [`HightlightPos::Start`].
    /// - The last element is [`HightlightPos::End`].
    /// - [`HightlightPos::InnerControlEnd`] appears only between `Start` and `End`.
    ///
    /// Note that this list can contain [`HightlightPos`] for multiple ranges like:
    /// `vec![Start, End, Start, InnerControlEnd, InnerControlEnd, End, Start, End]`
    positions: Vec<HightlightPos>,
}

/// A byte position to highlight in the [`Line::raw`].
/// It consists of three variants to highlight a line as properly as possible even if
/// the original line already contains some control sequences.
/// For example, if you want to highlight "go.to" in the following line:
///
/// ```text
/// This is [bold]Cargo[reset].toml.
///                  ^^       ^^^
/// ```
///
/// It will result in three `HighlightPos`:
/// 1. Start ... "g"
/// 2. InnerControlEnd ... "."
/// 3. End ... "m" (exclusive)
///
/// ```text
/// This is [bold]Car[1]go[reset][2].to[3]ml.
/// ```
///
/// `InnerControlEnd` is necessary to override styling made by original control sequences.
/// Therefore, `InnerControlEnd` can appear multiple times in a single highlight range.
#[derive(Debug)]
struct HightlightPos {
    index: usize,
    kind: HightlightPosKind,
}

impl HightlightPos {
    fn start(index: usize) -> Self {
        Self {
            index,
            kind: HightlightPosKind::Start,
        }
    }

    fn inner_control_end(index: usize) -> Self {
        Self {
            index,
            kind: HightlightPosKind::InnerControlEnd,
        }
    }

    fn end(index: usize) -> Self {
        Self {
            index,
            kind: HightlightPosKind::End,
        }
    }
}

#[derive(Debug)]
enum HightlightPosKind {
    Start,
    InnerControlEnd,
    End,
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use crate::line::Line;

    #[test]
    fn parse_line_with_escape_seqs_and_multi_byte_chars() {
        let s = "\u{1b}[1mHi\u{1b}[0m, 😀".to_string();
        let line = Line::new(s.clone());
        assert_eq!(&line.raw, &s);
        assert_eq!(&line.plain, "Hi, 😀");
        assert_eq!(
            &line.plain_to_raw,
            &[
                // (escape sequences)
                4, 5, // "Hi"
                // (escape sequences)
                10, 11, // ", "
                12, 13, 14, 15, // smile emoji
            ]
        );
    }
}
