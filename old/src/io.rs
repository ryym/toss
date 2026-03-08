use crate::AppResult;

/// A trait similar to [`std::io::Write`] but it accepts a `&str` instead of bytes.
pub(crate) trait WriteStr {
    fn write_all(&mut self, s: &str) -> AppResult<()>;
    fn flush(&mut self) -> AppResult<()>;
}

impl WriteStr for String {
    fn write_all(&mut self, s: &str) -> AppResult<()> {
        self.push_str(s);
        Ok(())
    }

    fn flush(&mut self) -> AppResult<()> {
        Ok(())
    }
}
