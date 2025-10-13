use ansi_control_codes::parser::{Token, TokenStream};

#[derive(Debug)]
pub(crate) struct Line {
    /// An original line including ANSI escape sequences.
    pub raw: String,

    /// A plain text line without escape sequences.
    pub plain: String,

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
    pub plain_to_raw: Vec<usize>,
}

impl Line {
    pub(crate) fn new(raw_line: String) -> Line {
        let mut plain = String::with_capacity(raw_line.len());
        let mut plain_to_raw = Vec::new();
        let mut i_raw = 0;
        for token in TokenStream::from(&raw_line) {
            match token {
                Token::ControlFunction(c) => {
                    // Allocate a string just to get the byte length of escape sequences :(
                    i_raw += String::from(c).len();
                }
                Token::String(s) => {
                    plain.push_str(s);
                    for _ in s.as_bytes() {
                        plain_to_raw.push(i_raw);
                        i_raw += 1;
                    }
                }
            }
        }
        Line {
            raw: raw_line,
            plain,
            plain_to_raw,
        }
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::Line;

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
