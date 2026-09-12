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

/// Exit code used for every failure that has no specific code of its own.
pub(crate) const DEFAULT_EXIT_CODE: i32 = 1;

/// An error to report to the user before exiting.
pub struct AppError {
    pub message: String,
    pub exit_code: i32,
}

impl AppError {
    pub fn new(message: impl Into<String>, exit_code: i32) -> Self {
        Self {
            message: message.into(),
            exit_code,
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
        Self::new(err.to_string(), DEFAULT_EXIT_CODE)
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
        self.map_err(|e| AppError::new(format!("{msg}: {e}"), DEFAULT_EXIT_CODE))
    }

    fn with_context<D: fmt::Display>(self, msg: impl FnOnce() -> D) -> Result<T, AppError> {
        self.map_err(|e| AppError::new(format!("{}: {e}", msg()), DEFAULT_EXIT_CODE))
    }
}
