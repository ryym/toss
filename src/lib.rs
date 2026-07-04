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
mod screen;
mod scroll;
mod search;

#[cfg(test)]
mod tests;

use std::ffi::OsString;
use std::fmt;
use std::io::{self, BufRead, BufReader, IsTerminal, Write};

use app::App;
use document::Document;
use pager::Pager;
use screen::{Screen, ScreenSize};

pub use screen::TermScreen;

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

/// Inputs required to run the toss pager pipeline.
///
/// `make_screen` is a factory rather than a value so the screen is only
/// constructed when actually needed — the `-F` short-circuit and the
/// help/version paths skip it, which lets the binary avoid acquiring raw
/// terminal mode in those cases.
struct RunConfig<R, W, S, MS>
where
    R: BufRead + Send + 'static,
    W: Write,
    S: Screen,
    MS: FnOnce() -> Result<S, AppError>,
{
    pub args: Vec<OsString>,
    pub terminal_size: ScreenSize,
    pub shell_lines: usize,
    pub instant_scroll: bool,
    pub stdin: R,
    pub stdin_is_terminal: bool,
    pub stdout: W,
    pub make_screen: MS,
}

/// Run the toss pipeline: parse CLI args, load the document, render the page.
pub fn run() -> Result<(), AppError> {
    let (w, h) = crossterm::terminal::size()
        .map_err(|e| AppError::new(format!("Error getting terminal size: {e}"), 1))?;
    let terminal_size = ScreenSize::new(w, h);

    let stdin = io::stdin();
    let stdin_is_terminal = stdin.is_terminal();

    let _ = run_inner(RunConfig {
        args: std::env::args_os().collect(),
        terminal_size,
        shell_lines: shell_lines(),
        instant_scroll: false,
        stdin: BufReader::new(stdin),
        stdin_is_terminal,
        stdout: io::stdout(),
        make_screen: || {
            TermScreen::new()
                .map_err(|e| AppError::new(format!("Error initializing terminal: {e}"), 1))
        },
    })?;

    Ok(())
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

/// How long to wait between pumps while blocking for streamed input at startup.
const STARTUP_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(1);

/// Block until the document has more than `max` lines or the input has ended.
/// Non-streaming sources are already complete, so this returns immediately.
fn wait_until_exceeds_or_complete(doc: &mut Document, max: usize) {
    loop {
        doc.pump();
        if doc.line_count() > max || doc.is_complete() {
            return;
        }
        std::thread::sleep(STARTUP_POLL_INTERVAL);
    }
}

/// Run the app. Return the screen only if it actually rendered a interactive pager.
/// For example, it doesn't render a pager for `--help`.
fn run_inner<R, W, S, MS>(cfg: RunConfig<R, W, S, MS>) -> Result<Option<S>, AppError>
where
    R: BufRead + Send + 'static,
    W: Write,
    S: Screen,
    MS: FnOnce() -> Result<S, AppError>,
{
    let stdin = cfg.stdin;
    let mut stdout = cfg.stdout;

    let _log_guard = logger::setup_file_logger()
        .map_err(|e| AppError::new(format!("Error setting up logger: {e}"), 1))?;

    let parsed = match cli::parse_from_args(cfg.args) {
        Ok(cli::Action::Run(args)) => args,
        Ok(cli::Action::Print(msg)) => {
            writeln!(stdout, "{msg}")
                .map_err(|e| AppError::new(format!("Error writing to stdout: {e}"), 1))?;
            return Ok(None);
        }
        Err(e) => return Err(AppError::new(format!("Error: {e}"), 1)),
    };

    let mut doc = if let Some(path) = parsed.file.as_ref() {
        log::debug!("Read file: {}", path.display());
        Document::from_file(path)
            .map_err(|e| AppError::new(format!("Error reading {}: {e}", path.display()), 1))?
    } else if !cfg.stdin_is_terminal {
        log::debug!("Read from stdin");
        Document::from_reader(stdin)
    } else {
        return Err(AppError::new("Usage: toss <file> OR command | toss", 1));
    };

    let size = cfg.terminal_size;
    let quit_if_one_screen = parsed.options.quit_if_one_screen;
    let one_screen = size.height().saturating_sub(cfg.shell_lines);

    // The pager assumes at least one line, so block until the first line is
    // available (or input ends). For non-streaming sources this returns at once.
    wait_until_exceeds_or_complete(&mut doc, 0);

    // With -F we must know whether everything fits on one screen. Read enough to
    // exceed a screen's worth of lines, or until the input ends.
    if quit_if_one_screen {
        wait_until_exceeds_or_complete(&mut doc, one_screen);
    }

    let mut pager = Pager::new(doc, parsed.options, size);

    if quit_if_one_screen && pager.fits_within(one_screen) {
        for i in 0..pager.doc_mut().line_count() {
            if let Some(line) = pager.doc_mut().line(i) {
                writeln!(stdout, "{}", line.raw())
                    .map_err(|e| AppError::new(format!("Error writing to stdout: {e}"), 1))?;
            }
        }
        return Ok(None);
    }

    let screen = (cfg.make_screen)()?;
    let mut app = App::new(screen, pager).map_err(|e| AppError::new(format!("{e}"), 1))?;
    if cfg.instant_scroll {
        app.set_instant_scroll();
    }
    app.run().map_err(|e| AppError::new(format!("{e}"), 1))?;

    Ok(Some(app.into_screen()))
}
