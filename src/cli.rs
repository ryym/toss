/// Command-line argument parsing.
use std::path::PathBuf;

use crate::options::{Options, SectionOptions};

const VERSION: &str = "0.0.0";

const HELP: &str = "\
Usage: toss [OPTIONS] [FILE]
       command | toss

A terminal pager.

Options:
      --header <N>           Pin the first N lines as a fixed header
      --section <REGEX>      Regex to identify section start lines for sticky headers
      --section-header <N>   Number of lines per section header (default 1)
  -h, --help                 Print help
  -v, --version              Print version";

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
    let mut section_pattern: Option<String> = None;
    let mut section_header_lines: Option<usize> = None;
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
            Long("section") => {
                section_pattern = Some(parser.value()?.parse()?);
            }
            Long("section-header") => {
                section_header_lines = Some(parser.value()?.parse()?);
            }
            Value(val) if file.is_none() => {
                file = Some(PathBuf::from(val));
            }
            _ => return Err(arg.unexpected()),
        }
    }

    if section_header_lines.is_some() && section_pattern.is_none() {
        return Err("--section-header requires --section".into());
    }

    let section = section_pattern
        .map(|pat| {
            let pattern =
                regex::Regex::new(&pat).map_err(|e| format!("invalid --section regex: {e}"))?;
            Ok::<_, String>(SectionOptions {
                pattern,
                header_lines: section_header_lines.unwrap_or(1),
            })
        })
        .transpose()
        .map_err(|e| e.to_string())?;

    Ok(Action::Run(Args {
        file,
        options: Options { header, section },
    }))
}
