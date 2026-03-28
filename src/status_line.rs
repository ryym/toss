/// Manages the content displayed on the status line.
pub struct StatusLine {
    content: String,
    dirty: bool,
}

impl StatusLine {
    pub fn new() -> Self {
        Self {
            content: ":".to_string(),
            dirty: false,
        }
    }

    /// Whether the content has been updated since the last render.
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Update the status line content.
    pub fn set_content(&mut self, content: String) {
        self.content = content;
        self.dirty = true;
    }

    /// Return the current content for rendering and clear the dirty flag.
    pub fn render(&mut self) -> &str {
        self.dirty = false;
        &self.content
    }
}
