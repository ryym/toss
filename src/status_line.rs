pub struct StatusLine;

impl StatusLine {
    pub fn new() -> Self {
        Self
    }

    pub fn render(&self) -> &str {
        ":"
    }
}
