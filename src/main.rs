use std::io::{self, IsTerminal};
use std::process;

use toss::{AppError, RunConfig, TermScreen, run};

fn main() {
    match run_main() {
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

fn run_main() -> Result<(), AppError> {
    let terminal_size = crossterm::terminal::size()
        .map_err(|e| AppError::new(format!("Error getting terminal size: {e}"), 1))?;

    let stdin = io::stdin();
    let stdin_is_terminal = stdin.is_terminal();

    let _ = run(RunConfig {
        args: std::env::args_os().collect(),
        terminal_size,
        shell_lines: shell_lines(),
        instant_scroll: false,
        stdin: stdin.lock(),
        stdin_is_terminal,
        stdout: io::stdout(),
        make_screen: || {
            TermScreen::new()
                .map_err(|e| AppError::new(format!("Error initializing terminal: {e}"), 1))
        },
    })?;

    Ok(())
}
