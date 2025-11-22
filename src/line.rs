use crate::terminal;

/// A line in target text. It can be decorated with ANSI escape sequences.
#[derive(Debug, Default)]
pub(crate) struct Line {
    /// An original line including ANSI escape sequences.
    raw: String,

    /// A plain text line without escape sequences.
    plain: String,

    /// A byte index mapping from each character in `plain` to `raw`. Example:
    /// - raw: `\xb[1mHi\xb[0m, 😀` ("Hi" is bold)
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
