use std::ffi::OsString;
use std::fs::File;
use std::io::{self, BufRead, BufReader, IsTerminal, Read, Write};

use crate::app::App;
use crate::document::Document;
use crate::pager::Pager;
use crate::screen::{Screen, ScreenSize, TermScreen};
use crate::{AppError, Context, cli, logger};

/// Run the toss pipeline: parse CLI args, load the document, render the page.
pub fn run() -> Result<(), AppError> {
    let stdin = io::stdin();
    let stdin_is_terminal = stdin.is_terminal();
    let stdout = io::stdout();
    let stdout_is_terminal = stdout.is_terminal();

    run_with(RunConfig {
        args: std::env::args_os().collect(),
        get_terminal_size: || {
            let (w, h) = crossterm::terminal::size().context("Error getting terminal size")?;
            Ok(ScreenSize::new(w, h))
        },
        shell_lines: shell_lines(),
        instant_scroll: false,
        stdin: BufReader::new(stdin),
        stdin_is_terminal,
        stdout,
        stdout_is_terminal,
        make_screen: TermScreen::new,
        wait_for_all_input: false,
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

/// Inputs required to run the toss pager pipeline.
pub(crate) struct RunConfig<R, W, S, MS, TS>
where
    R: BufRead + Send + 'static,
    W: Write,
    S: Screen,
    MS: FnOnce(W) -> Result<S, AppError>,
    TS: FnOnce() -> Result<ScreenSize, AppError>,
{
    pub args: Vec<OsString>,
    pub shell_lines: usize,
    pub instant_scroll: bool,
    pub stdin: R,
    pub stdin_is_terminal: bool,
    pub stdout: W,
    pub stdout_is_terminal: bool,
    /// A factory so that the terminal is left alone, without raw mode,
    /// unless the pager actually runs.
    pub make_screen: MS,
    /// A factory so that toss also runs where there is no terminal to ask.
    pub get_terminal_size: TS,
    /// Block until the whole input has been read before starting the pager. Used for testing.
    pub wait_for_all_input: bool,
}

/// Run the app with the given config. Return the screen only if it actually rendered
/// a interactive pager. For example, it doesn't render a pager for `--help`.
pub(crate) fn run_with<R, W, S, MS, TS>(cfg: RunConfig<R, W, S, MS, TS>) -> Result<(), AppError>
where
    R: BufRead + Send + 'static,
    W: Write,
    S: Screen,
    MS: FnOnce(W) -> Result<S, AppError>,
    TS: FnOnce() -> Result<ScreenSize, AppError>,
{
    let _log_guard = logger::setup_file_logger()?;
    let mut stdin = cfg.stdin;
    let mut stdout = cfg.stdout;

    // Parse CLI arguments.
    let parsed = match cli::parse_from_args(cfg.args)? {
        cli::Action::Run(args) => args,
        cli::Action::Print(msg) => {
            writeln!(stdout, "{msg}").context("Error writing to stdout")?;
            return Ok(());
        }
    };

    // Relay the input like `cat` if there is no screen to paginate on.
    // Otherwise piping like `toss file | cmd` hangs.
    if !cfg.stdout_is_terminal {
        log::debug!("Relay the input as stdout is not a terminal");
        return match parsed.file.as_ref() {
            Some(path) => {
                let mut file = File::open(path)
                    .with_context(|| format!("Error reading {}", path.display()))?;
                relay(&mut file, &mut stdout)
            }
            None if !cfg.stdin_is_terminal => relay(&mut stdin, &mut stdout),
            None => Err(usage_error()),
        };
    }

    // Construct a document to paginate.
    let mut doc = if let Some(path) = parsed.file.as_ref() {
        log::debug!("Read file: {}", path.display());
        Document::from_file(path).with_context(|| format!("Error reading {}", path.display()))?
    } else if !cfg.stdin_is_terminal {
        log::debug!("Read from stdin");
        Document::from_reader(stdin)
    } else {
        return Err(usage_error());
    };

    let size = (cfg.get_terminal_size)()?;
    let one_screen = size.height().saturating_sub(cfg.shell_lines);

    if cfg.wait_for_all_input {
        wait_until_exceeds_or_complete(&mut doc, usize::MAX);
    } else if parsed.quit_if_one_screen {
        // With -F we must know whether everything fits on one screen. Read enough to
        // exceed a screen's worth of lines, or until the input ends.
        wait_until_exceeds_or_complete(&mut doc, one_screen);
    } else {
        // The pager assumes at least one line, so block until the first line is
        // available (or input ends). For non-streaming sources this returns at once.
        wait_until_exceeds_or_complete(&mut doc, 0);
    }

    let mut pager = Pager::new(doc, parsed.options, size);

    let stream_error = if parsed.quit_if_one_screen && pager.fits_within(one_screen) {
        pager
            .print_all(&mut stdout)
            .context("Error writing to stdout")?
    } else {
        let screen = (cfg.make_screen)(stdout)?;
        let mut app = App::new(screen, pager)?;
        if cfg.instant_scroll {
            app.set_instant_scroll();
        }
        app.run()?
    };
    if let Some(e) = stream_error {
        return Err(AppError::new(format!("Error reading stdin: {e}")));
    }

    Ok(())
}

fn usage_error() -> AppError {
    AppError::new("Usage: toss <file> OR command | toss")
}

/// Copy the input to the output as is, without paginating it.
///
/// The bytes are relayed rather than printed through [`Document`] so that the output
/// stays identical to the input (trailing newline, CRLF, invalid UTF-8) and so that
/// an endless stream keeps flowing instead of being buffered until EOF.
fn relay(src: &mut impl Read, dst: &mut impl Write) -> Result<(), AppError> {
    let result = io::copy(src, dst).and_then(|_| dst.flush());
    match result {
        Ok(()) => Ok(()),
        // The downstream command exited first, as in `toss file | head`. The Rust
        // runtime sets SIGPIPE to SIG_IGN (https://github.com/rust-lang/rust/issues/97889),
        // so this surfaces as an error instead of killing the process. It is a normal
        // way for a pipeline to end, so report success.
        Err(e) if e.kind() == io::ErrorKind::BrokenPipe => Ok(()),
        Err(e) => Err(e).context("Error relaying the input to stdout"),
    }
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
