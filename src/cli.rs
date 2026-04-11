//! Command-line argument parsing.

use std::path::PathBuf;

use crate::options::{Options, SectionOptions};

const VERSION: &str = "0.0.0";

const HELP: &str = "\
Usage: toss [OPTIONS] [FILE]
       command | toss

A terminal pager.

Options:
  -F, --quit-if-one-screen   Quit if the entire content fits on one screen
      --header <N>           Pin the first N lines as a fixed header
      --section <REGEX>      Regex to identify section start lines for sticky headers
      --section-header <N>   Number of lines per section header (default 1)
  -h, --help                 Print help
  -v, --version              Print version

Environment variables:
  TOSS_SHELL_LINES   Number of shell prompt lines to reserve when using -F
                     (falls back to LESS_SHELL_LINES, default 1)";

/// Parsed command-line action.
#[derive(Debug)]
pub enum Action {
    /// Run the pager.
    Run(Args),
    /// Print a message and exit.
    Print(String),
}

/// Parsed command-line arguments.
#[derive(Debug)]
pub struct Args {
    pub file: Option<PathBuf>,
    pub options: Options,
}

/// Parse command-line arguments from the environment.
pub fn parse_args() -> Result<Action, lexopt::Error> {
    parse_from(lexopt::Parser::from_env())
}

/// Parse command-line arguments from the given argument list.
#[cfg(test)]
pub fn parse_from_args<I>(args: I) -> Result<Action, lexopt::Error>
where
    I: IntoIterator,
    I::Item: Into<std::ffi::OsString>,
{
    parse_from(lexopt::Parser::from_iter(args))
}

fn parse_from(mut parser: lexopt::Parser) -> Result<Action, lexopt::Error> {
    use lexopt::prelude::*;

    let mut file = None;
    let mut quit_if_one_screen = false;
    let mut header = 0;
    let mut section_pattern: Option<String> = None;
    let mut section_header_lines: Option<usize> = None;

    while let Some(arg) = parser.next()? {
        match arg {
            Short('F') | Long("quit-if-one-screen") => {
                quit_if_one_screen = true;
            }
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
                let n: usize = parser.value()?.parse()?;
                if n == 0 {
                    return Err("--section-header must be at least 1".into());
                }
                section_header_lines = Some(n);
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
        options: Options {
            quit_if_one_screen,
            header,
            section,
        },
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Action {
        let full: Vec<&str> = std::iter::once(&"toss")
            .chain(args.iter())
            .copied()
            .collect();
        parse_from_args(full).unwrap()
    }

    fn parse_err(args: &[&str]) -> String {
        let full: Vec<&str> = std::iter::once(&"toss")
            .chain(args.iter())
            .copied()
            .collect();
        parse_from_args(full).unwrap_err().to_string()
    }

    fn unwrap_run(action: Action) -> Args {
        match action {
            Action::Run(args) => args,
            Action::Print(msg) => panic!("expected Run, got Print: {msg}"),
        }
    }

    fn unwrap_print(action: Action) -> String {
        match action {
            Action::Print(msg) => msg,
            Action::Run(_) => panic!("expected Print, got Run"),
        }
    }

    #[test]
    fn no_args() {
        let args = unwrap_run(parse(&[]));
        assert!(args.file.is_none());
        assert!(!args.options.quit_if_one_screen);
        assert_eq!(args.options.header, 0);
        assert!(args.options.section.is_none());
    }

    #[test]
    fn file_arg() {
        let args = unwrap_run(parse(&["foo.txt"]));
        assert_eq!(args.file.unwrap(), PathBuf::from("foo.txt"));
    }

    #[test]
    fn quit_if_one_screen_short() {
        let args = unwrap_run(parse(&["-F"]));
        assert!(args.options.quit_if_one_screen);
    }

    #[test]
    fn quit_if_one_screen_long() {
        let args = unwrap_run(parse(&["--quit-if-one-screen"]));
        assert!(args.options.quit_if_one_screen);
    }

    #[test]
    fn header_option() {
        let args = unwrap_run(parse(&["--header", "3"]));
        assert_eq!(args.options.header, 3);
    }

    #[test]
    fn section_option() {
        let args = unwrap_run(parse(&["--section", "^##"]));
        let section = args.options.section.unwrap();
        assert_eq!(section.pattern.as_str(), "^##");
        assert_eq!(section.header_lines, 1);
    }

    #[test]
    fn section_with_header_lines() {
        let args = unwrap_run(parse(&["--section", "^##", "--section-header", "2"]));
        let section = args.options.section.unwrap();
        assert_eq!(section.header_lines, 2);
    }

    #[test]
    fn section_header_without_section_is_error() {
        let err = parse_err(&["--section-header", "2"]);
        assert!(err.contains("--section-header requires --section"), "{err}");
    }

    #[test]
    fn section_header_zero_is_error() {
        let err = parse_err(&["--section", "^##", "--section-header", "0"]);
        assert!(err.contains("at least 1"), "{err}");
    }

    #[test]
    fn help_flag() {
        let msg = unwrap_print(parse(&["-h"]));
        assert!(msg.contains("Usage:"));
    }

    #[test]
    fn version_flag() {
        let msg = unwrap_print(parse(&["-v"]));
        assert!(msg.starts_with("toss "));
    }

    #[test]
    fn unknown_flag_is_error() {
        assert!(parse_from_args(["toss", "--unknown"]).is_err());
    }
}
