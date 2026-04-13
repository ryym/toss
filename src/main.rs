mod ansi;
mod app;
mod cli;
mod document;
mod line;
mod line_editor;
mod logger;
mod options;
mod page;
mod pager;
mod render;
mod screen;
mod scroll;
mod search;
mod status_line;

#[cfg(test)]
mod tests;

use std::fmt;
use std::io::{self, IsTerminal};
use std::path::PathBuf;
use std::process;

use app::App;
use document::Document;
use page::Page;
use screen::TermScreen;

struct AppError {
    message: String,
    exit_code: i32,
}

impl AppError {
    fn new(message: impl Into<String>, exit_code: i32) -> Self {
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

fn main() {
    match run() {
        Ok(()) => {}
        Err(e) => {
            eprintln!("{e}");
            process::exit(e.exit_code);
        }
    }
}

/// Read the number of shell prompt lines to reserve from environment variables.
/// Checks TOSS_SHELL_LINES first, then LESS_SHELL_LINES, defaulting to 1.
fn shell_lines() -> usize {
    std::env::var("TOSS_SHELL_LINES")
        .or_else(|_| std::env::var("LESS_SHELL_LINES"))
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1)
}

fn run() -> Result<(), AppError> {
    let _log_guard = logger::setup_file_logger()
        .map_err(|e| AppError::new(format!("Error setting up logger: {e}"), 1))?;

    let args = match cli::parse_args() {
        Ok(cli::Action::Run(args)) => args,
        Ok(cli::Action::Print(msg)) => {
            println!("{msg}");
            return Ok(());
        }
        Err(e) => return Err(AppError::new(format!("Error: {e}"), 1)),
    };

    let doc = build_document(args.file)?;

    let (w, h) = crossterm::terminal::size()
        .map_err(|e| AppError::new(format!("Error getting terminal size: {e}"), 1))?;
    let mut page = Page::new(doc, &args.options, w as usize, h as usize);

    if args.options.quit_if_one_screen && page.content_fits_on_screen(h as usize, shell_lines()) {
        for i in 0..page.doc.line_count() {
            if let Some(line) = page.doc.line(i) {
                println!("{}", line.raw());
            }
        }
        return Ok(());
    }

    let screen = TermScreen::new()
        .map_err(|e| AppError::new(format!("Error initializing terminal: {e}"), 1))?;

    let mut app = App::new(screen, page).map_err(|e| AppError::new(format!("{e}"), 1))?;
    app.run().map_err(|e| AppError::new(format!("{e}"), 1))?;

    Ok(())
}

fn build_document(path: Option<PathBuf>) -> Result<Document, AppError> {
    let doc = if let Some(path) = &path {
        log::debug!("Read file: {}", path.display());
        Document::from_file(path)
            .map_err(|e| AppError::new(format!("Error reading {}: {e}", path.display()), 1))?
    } else if !io::stdin().is_terminal() {
        log::debug!("Read from stdin");
        Document::from_reader(&mut io::stdin().lock())
            .map_err(|e| AppError::new(format!("Error reading stdin: {e}"), 1))?
    } else {
        return Err(AppError::new("Usage: toss <file> OR command | toss", 1));
    };
    Ok(doc)
}
