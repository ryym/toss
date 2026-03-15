/// Command-line argument parsing.
use std::path::PathBuf;

use crate::options::Options;

const VERSION: &str = "0.0.0";

const HELP: &str = "\
Usage: toss [OPTIONS] [FILE]
       command | toss

A terminal pager.

Options:
      --header <N>  Pin the first N lines as a fixed header
  -h, --help        Print help
  -v, --version     Print version";

/// Parsed command-line action.
pub enum Action {
    /// Run the pager.
    Run(Args),
    /// Print a message and exit.
    Print(String),
}

/// Parsed command-line arguments.
pub struct Args {
    pub file: Option<PathBuf>,
    pub options: Options,
}

pub fn parse_args() -> Result<Action, lexopt::Error> {
    use lexopt::prelude::*;

    let mut file = None;
    let mut header = 0;
    let mut parser = lexopt::Parser::from_env();

    while let Some(arg) = parser.next()? {
        match arg {
            Short('h') | Long("help") => return Ok(Action::Print(HELP.to_string())),
            Short('v') | Long("version") => {
                return Ok(Action::Print(format!("toss {VERSION}")));
            }
            Long("header") => {
                header = parser.value()?.parse()?;
            }
            Value(val) if file.is_none() => {
                file = Some(PathBuf::from(val));
            }
            _ => return Err(arg.unexpected()),
        }
    }

    Ok(Action::Run(Args {
        file,
        options: Options { header },
    }))
}
