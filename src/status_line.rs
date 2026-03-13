/// Manages the content displayed on the status line.
pub struct StatusLine {
    content: String,
}

impl StatusLine {
    pub fn new() -> Self {
        Self {
            content: ":".to_string(),
        }
    }

    /// Update the status line content.
    pub fn set_content(&mut self, content: String) {
        self.content = content;
    }

    /// Return the current content for rendering.
    pub fn render(&self) -> &str {
        &self.content
    }
}
