/// Command-line argument parsing.
use std::path::PathBuf;

const VERSION: &str = "0.0.0";

/// Parsed command-line action.
pub enum Action {
    /// Run the pager.
    Run(Args),
    /// Print version and exit.
    PrintVersion,
}

/// Parsed command-line arguments.
pub struct Args {
    pub file: Option<PathBuf>,
}

pub fn parse_args() -> Result<Action, lexopt::Error> {
    use lexopt::prelude::*;

    let mut file = None;
    let mut parser = lexopt::Parser::from_env();

    while let Some(arg) = parser.next()? {
        match arg {
            Short('v') | Long("version") => return Ok(Action::PrintVersion),
            Value(val) if file.is_none() => {
                file = Some(PathBuf::from(val));
            }
            _ => return Err(arg.unexpected()),
        }
    }

    Ok(Action::Run(Args { file }))
}

/// Return the version string.
pub fn version() -> &'static str {
    VERSION
}
