use unicode_width::UnicodeWidthChar;

/// A single line of text from the document.
#[derive(Debug, Clone)]
pub struct Line {
    text: String,
}

impl Line {
    pub fn new(text: String) -> Self {
        Self { text }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    /// Compute wrap positions for this line at the given screen width.
    /// Returns the byte offsets where each wrapped row starts.
    /// The first element is always 0.
    pub fn wrap_positions(&self, width: usize) -> Vec<usize> {
        if width == 0 {
            return vec![0];
        }

        let mut positions = vec![0];
        let mut col = 0;

        for (byte_idx, ch) in self.text.char_indices() {
            let ch_width = ch.width().unwrap_or(0);
            if col + ch_width > width && col > 0 {
                positions.push(byte_idx);
                col = ch_width;
            } else {
                col += ch_width;
            }
        }

        positions
    }

    /// Number of screen rows this line occupies at the given width.
    pub fn row_count(&self, width: usize) -> usize {
        self.wrap_positions(width).len()
    }

    /// Get the text slice for a specific wrapped row.
    pub fn wrap_row_text(&self, width: usize, wrap_index: usize) -> &str {
        let positions = self.wrap_positions(width);
        let start = positions[wrap_index];
        let end = positions
            .get(wrap_index + 1)
            .copied()
            .unwrap_or(self.text.len());
        &self.text[start..end]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrap_short_line() {
        let line = Line::new("hello".into());
        assert_eq!(line.row_count(80), 1);
        assert_eq!(line.wrap_row_text(80, 0), "hello");
    }

    #[test]
    fn wrap_exact_width() {
        let line = Line::new("abcde".into());
        assert_eq!(line.row_count(5), 1);
        assert_eq!(line.wrap_row_text(5, 0), "abcde");
    }

    #[test]
    fn wrap_overflow() {
        let line = Line::new("abcdefgh".into());
        assert_eq!(line.row_count(5), 2);
        assert_eq!(line.wrap_row_text(5, 0), "abcde");
        assert_eq!(line.wrap_row_text(5, 1), "fgh");
    }

    #[test]
    fn wrap_multiple_rows() {
        let line = Line::new("abcdefghijklm".into());
        assert_eq!(line.row_count(5), 3);
        assert_eq!(line.wrap_row_text(5, 0), "abcde");
        assert_eq!(line.wrap_row_text(5, 1), "fghij");
        assert_eq!(line.wrap_row_text(5, 2), "klm");
    }

    #[test]
    fn wrap_wide_chars() {
        // Each CJK char is 2 columns wide
        let line = Line::new("あいうえお".into());
        // width=6: "あいう" (6 cols), "えお" (4 cols)
        assert_eq!(line.row_count(6), 2);
        assert_eq!(line.wrap_row_text(6, 0), "あいう");
        assert_eq!(line.wrap_row_text(6, 1), "えお");
    }

    #[test]
    fn wrap_wide_char_at_boundary() {
        // width=5: "あい" (4 cols), then "う" (2 cols) won't fit -> wrap
        let line = Line::new("あいう".into());
        assert_eq!(line.row_count(5), 2);
        assert_eq!(line.wrap_row_text(5, 0), "あい");
        assert_eq!(line.wrap_row_text(5, 1), "う");
    }

    #[test]
    fn empty_line() {
        let line = Line::new(String::new());
        assert_eq!(line.row_count(80), 1);
        assert_eq!(line.wrap_row_text(80, 0), "");
    }
}
