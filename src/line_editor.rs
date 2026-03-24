/// Single-line text editor for the search prompt.
pub struct LineEditor {
    input: Vec<char>,
    cursor: usize,
}

impl LineEditor {
    pub fn new() -> Self {
        Self {
            input: Vec::new(),
            cursor: 0,
        }
    }

    /// Insert a character at the current cursor position.
    pub fn insert(&mut self, ch: char) {
        self.input.insert(self.cursor, ch);
        self.cursor += 1;
    }

    /// Delete the character before the cursor.
    pub fn backspace(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
            self.input.remove(self.cursor);
        }
    }

    /// Return the current input as a string.
    pub fn input(&self) -> String {
        self.input.iter().collect()
    }

    /// Return the input with a block cursor character at the current position.
    pub fn input_with_cursor(&self) -> String {
        let mut result: String = self.input[..self.cursor].iter().collect();
        result.push('█');
        result.extend(&self.input[self.cursor..]);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_and_backspace() {
        let mut editor = LineEditor::new();
        editor.insert('a');
        editor.insert('b');
        editor.insert('c');
        assert_eq!(editor.input(), "abc");
        assert_eq!(editor.cursor, 3);

        editor.backspace();
        assert_eq!(editor.input(), "ab");
        assert_eq!(editor.cursor, 2);
    }

    #[test]
    fn input_with_cursor_shows_block_at_position() {
        let mut editor = LineEditor::new();
        assert_eq!(editor.input_with_cursor(), "█");

        editor.insert('a');
        editor.insert('b');
        assert_eq!(editor.input_with_cursor(), "ab█");

        editor.backspace();
        assert_eq!(editor.input_with_cursor(), "a█");
    }

    #[test]
    fn backspace_at_start_does_nothing() {
        let mut editor = LineEditor::new();
        editor.backspace();
        assert_eq!(editor.input(), "");
        assert_eq!(editor.cursor, 0);
    }
}
