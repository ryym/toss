mod ansi;
mod app;
mod cli;
mod document;
mod line;
mod line_editor;
mod logger;
mod options;
mod pager;
mod renderer;
mod run;
mod screen;
mod scroll;
mod search;

#[cfg(test)]
mod tests;

use std::fmt;
use std::io;

pub use run::run;

/// An error to report to the user before exiting.
#[derive(Debug)]
pub struct AppError {
    pub message: String,
    pub exit_code: i32,
}

impl AppError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            exit_code: 1, // exit with 1 by default.
        }
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl From<io::Error> for AppError {
    fn from(err: io::Error) -> Self {
        Self::new(err.to_string())
    }
}

/// Attach a human-readable context to an error and turn it into an [`AppError`].
pub trait Context<T> {
    /// Prepend `msg` to the error message, as in `"{msg}: {err}"`.
    fn context(self, msg: impl fmt::Display) -> Result<T, AppError>;

    /// Same as [`Context::context`], but builds the message only on error.
    fn with_context<D: fmt::Display>(self, msg: impl FnOnce() -> D) -> Result<T, AppError>;
}

impl<T, E: fmt::Display> Context<T> for Result<T, E> {
    fn context(self, msg: impl fmt::Display) -> Result<T, AppError> {
        self.map_err(|e| AppError::new(format!("{msg}: {e}")))
    }

    fn with_context<D: fmt::Display>(self, msg: impl FnOnce() -> D) -> Result<T, AppError> {
        self.map_err(|e| AppError::new(format!("{}: {e}", msg())))
    }
}
