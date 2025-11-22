//! This module parses text on terminal and recognizes control characters in it.
//! The parser roughly follows the spec of ECMA-48. References:
//! - <https://www.ecma-international.org/wp-content/uploads/ECMA-48_5th_edition_june_1991.pdf>
//! - <https://en.wikipedia.org/wiki/ANSI_escape_code>
//! - <https://en.wikipedia.org/wiki/C0_and_C1_control_codes>

use std::{ops::Range, str::CharIndices};

pub(crate) fn parse_text(text: &str) -> impl Iterator<Item = Text<'_>> {
    TextIter::new(text)
}

#[derive(Debug, PartialEq)]
pub(crate) enum Text<'t> {
    Control(&'t str),
    Plain(&'t str),
}

struct TextIter<'t> {
    text: &'t str,
    cursor: usize,
    next_control_range: Option<Range<usize>>,
}

impl<'t> TextIter<'t> {
    fn new(text: &'t str) -> Self {
        Self {
            text,
            cursor: 0,
            next_control_range: None,
        }
    }
}

impl<'t> Iterator for TextIter<'t> {
    type Item = Text<'t>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.text.len() <= self.cursor {
            return None;
        }
        if let Some(range) = self.next_control_range.take() {
            self.cursor = range.end;
            return Some(Text::Control(&self.text[range]));
        }
        match find_next_control_range(&self.text[self.cursor..]) {
            Some(range) => {
                let range = (self.cursor + range.start)..(self.cursor + range.end);
                if self.cursor == range.start {
                    self.cursor = range.end;
                    Some(Text::Control(&self.text[range]))
                } else {
                    let plain = &self.text[self.cursor..range.start];
                    self.next_control_range = Some(range);
                    Some(Text::Plain(plain))
                }
            }
            None => {
                let plain = &self.text[self.cursor..];
                self.cursor = self.text.len();
                Some(Text::Plain(plain))
            }
        }
    }
}

const ESC: char = '\u{1b}';

fn find_next_control_range(text: &str) -> Option<Range<usize>> {
    let mut chars = text.char_indices();
    while let Some((i_start, c_start)) = chars.next() {
        if c_start == ESC {
            let seq_len = match chars.next() {
                None => 0,
                Some((_, c)) => {
                    // C1 (0x40..=0x5f)
                    if ('@'..='_').contains(&c) {
                        match c {
                            // CSI
                            '[' => c.len_utf8() + consume_csi_sequence(&mut chars),
                            // Opening delimiter of a control string (DCS, SOS, OSC, PM, APC)
                            'P' | 'X' | ']' | '^' | '_' => {
                                c.len_utf8() + consume_control_string(&mut chars)
                            }
                            // Other control function or unexpected character
                            _ => c.len_utf8(),
                        }
                    }
                    // Independent control functions (0x60..=0x7e) or unexpected character
                    else {
                        c.len_utf8()
                    }
                }
            };
            let i_end = i_start + c_start.len_utf8() + seq_len;
            return Some(i_start..i_end);
        }
        // C0 other than ESC (0x00..=0x1f)
        else if ('\u{00}'..='\u{1f}').contains(&c_start) {
            return Some(i_start..(i_start + c_start.len_utf8()));
        }
    }
    None
}

fn consume_csi_sequence(chars: &mut CharIndices) -> usize {
    let mut len = 0;
    for (_, c) in chars {
        len += c.len_utf8();
        // parameter byte (0x30..=0x3f) or intermediate byte (0x20..=0x2f)
        if ('0'..='?').contains(&c) || (' '..='/').contains(&c) {
            continue;
        }
        // final byte (0x40..=0x7e) or any invalid character
        break;
    }
    len
}

fn consume_control_string(chars: &mut CharIndices) -> usize {
    let mut len = 0;
    let mut is_prev_esc = false;
    for (_, c) in chars {
        len += c.len_utf8();
        if is_prev_esc && c == '\\' {
            break; // '0x1b 0x5c' == ST (string terminator) found
        } else {
            is_prev_esc = c == ESC;
        }
    }
    len
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use crate::terminal::{self, Text};

    fn parse(text: &str) -> Vec<Text<'_>> {
        terminal::parse_text(text).collect()
    }

    macro_rules! assert_parse {
        ($input:expr, $expected:expr) => {
            assert_eq!(parse($input), $expected, "original: {:?}", $input)
        };
    }

    #[test]
    fn parse_empty() {
        assert_parse!("", vec![]);
    }

    #[test]
    fn parse_plain_text() {
        assert_parse!("abcde", vec![Text::Plain("abcde")]);
    }

    #[test]
    fn parse_del() {
        // The Standard does not define DEL as a control character.
        assert_parse!("\u{7f}", vec![Text::Plain("\u{7f}")]);
    }

    #[test]
    fn parse_single_control_char() {
        assert_parse!("\0", vec![Text::Control("\u{00}")]);
        assert_parse!("\n", vec![Text::Control("\u{0a}")]);
        assert_parse!("\u{1b}", vec![Text::Control("\u{1b}")]);
    }

    #[test]
    fn parse_single_control_seq() {
        assert_parse!("\u{1b}[s", vec![Text::Control("\u{1b}[s")]);
        assert_parse!("\u{1b}[1;2H", vec![Text::Control("\u{1b}[1;2H")]);
        assert_parse!("\u{1b}[0m", vec![Text::Control("\u{1b}[0m")]);
        assert_parse!(
            "\u{1b}[38;2;255;255;255m",
            vec![Text::Control("\u{1b}[38;2;255;255;255m")]
        );
    }

    #[test]
    fn parse_invalid_control_seq() {
        assert_parse!("\u{1b}[あ", vec![Text::Control("\u{1b}[あ")]);
    }

    #[test]
    fn parse_control_string() {
        assert_parse!(
            "\u{1b}]abc\u{1b}\u{5c}",
            vec![Text::Control("\u{1b}]abc\u{1b}\u{5c}")]
        );
    }

    #[test]
    fn parse_consecutive_controls() {
        assert_parse!(
            "\0\t\n",
            vec![
                Text::Control("\0"),
                Text::Control("\t"),
                Text::Control("\n")
            ]
        );
        assert_parse!(
            "\u{1b}[7m\u{1b}[27;0m\n",
            vec![
                Text::Control("\u{1b}[7m"),
                Text::Control("\u{1b}[27;0m"),
                Text::Control("\n")
            ]
        );
    }

    #[test]
    fn parse_mixed_text() {
        assert_parse!(
            "\u{1b}[7mabc\u{1b}[27;0m\n",
            vec![
                Text::Control("\u{1b}[7m"),
                Text::Plain("abc"),
                Text::Control("\u{1b}[27;0m"),
                Text::Control("\n")
            ]
        );
        assert_parse!(
            "c0: \u{00} - \u{1f}, independent: \u{1b}\u{60} - \u{1b}\u{7e}",
            vec![
                Text::Plain("c0: "),
                Text::Control("\u{00}"),
                Text::Plain(" - "),
                Text::Control("\u{1f}"),
                Text::Plain(", independent: "),
                Text::Control("\u{1b}\u{60}"),
                Text::Plain(" - "),
                Text::Control("\u{1b}\u{7e}"),
            ]
        );
    }
}
