pub enum LineEdit {
    /// Insert a character at the current cursor position.
    AddChar(char),
    /// Delete the character before the cursor.
    DeleteCharBeforeCursor,
    /// Move the cursor one position to the left.
    MoveCursorLeft,
    /// Move the cursor one position to the right.
    MoveCursorRight,
}

/// Single-line text editor for the search prompt.
pub struct LineEditor {
    input: Vec<char>,
    cursor: usize,
}

impl Default for LineEditor {
    fn default() -> Self {
        Self::new()
    }
}

impl LineEditor {
    pub fn new() -> Self {
        Self {
            input: Vec::new(),
            cursor: 0,
        }
    }

    pub fn edit(&mut self, edit: LineEdit) {
        match edit {
            LineEdit::AddChar(ch) => {
                self.input.insert(self.cursor, ch);
                self.cursor += 1;
            }
            LineEdit::DeleteCharBeforeCursor => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                    self.input.remove(self.cursor);
                }
            }
            LineEdit::MoveCursorLeft => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                }
            }
            LineEdit::MoveCursorRight => {
                if self.cursor < self.input.len() {
                    self.cursor += 1;
                }
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        self.input.is_empty()
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
        editor.edit(LineEdit::AddChar('a'));
        editor.edit(LineEdit::AddChar('b'));
        editor.edit(LineEdit::AddChar('c'));
        assert_eq!(editor.input(), "abc");
        assert_eq!(editor.cursor, 3);

        editor.edit(LineEdit::DeleteCharBeforeCursor);
        assert_eq!(editor.input(), "ab");
        assert_eq!(editor.cursor, 2);
    }

    #[test]
    fn input_with_cursor_shows_block_at_position() {
        let mut editor = LineEditor::new();
        assert_eq!(editor.input_with_cursor(), "█");

        editor.edit(LineEdit::AddChar('a'));
        editor.edit(LineEdit::AddChar('b'));
        assert_eq!(editor.input_with_cursor(), "ab█");

        editor.edit(LineEdit::DeleteCharBeforeCursor);
        assert_eq!(editor.input_with_cursor(), "a█");
    }

    #[test]
    fn move_left_and_right() {
        let mut editor = LineEditor::new();
        editor.edit(LineEdit::AddChar('a'));
        editor.edit(LineEdit::AddChar('b'));
        editor.edit(LineEdit::AddChar('c'));
        assert_eq!(editor.cursor, 3);

        editor.edit(LineEdit::MoveCursorLeft);
        assert_eq!(editor.cursor, 2);
        assert_eq!(editor.input_with_cursor(), "ab█c");

        editor.edit(LineEdit::MoveCursorLeft);
        assert_eq!(editor.cursor, 1);
        assert_eq!(editor.input_with_cursor(), "a█bc");

        // Insert at middle position.
        editor.edit(LineEdit::AddChar('x'));
        assert_eq!(editor.input(), "axbc");
        assert_eq!(editor.input_with_cursor(), "ax█bc");

        editor.edit(LineEdit::MoveCursorRight);
        assert_eq!(editor.input_with_cursor(), "axb█c");

        // Backspace at middle position.
        editor.edit(LineEdit::DeleteCharBeforeCursor);
        assert_eq!(editor.input(), "axc");
        assert_eq!(editor.input_with_cursor(), "ax█c");
    }

    #[test]
    fn move_left_at_start_does_nothing() {
        let mut editor = LineEditor::new();
        editor.edit(LineEdit::MoveCursorLeft);
        assert_eq!(editor.cursor, 0);

        editor.edit(LineEdit::AddChar('a'));
        editor.edit(LineEdit::MoveCursorLeft);
        editor.edit(LineEdit::MoveCursorLeft); // Already at 0
        assert_eq!(editor.cursor, 0);
    }

    #[test]
    fn move_right_at_end_does_nothing() {
        let mut editor = LineEditor::new();
        editor.edit(LineEdit::MoveCursorRight);
        assert_eq!(editor.cursor, 0);

        editor.edit(LineEdit::AddChar('a'));
        editor.edit(LineEdit::MoveCursorRight); // Already at end
        assert_eq!(editor.cursor, 1);
    }

    #[test]
    fn backspace_at_start_does_nothing() {
        let mut editor = LineEditor::new();
        editor.edit(LineEdit::DeleteCharBeforeCursor);
        assert_eq!(editor.input(), "");
        assert_eq!(editor.cursor, 0);
    }
}
