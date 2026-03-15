/// Command-line argument parsing.
use std::path::PathBuf;

/// Parsed command-line arguments.
pub struct Args {
    pub file: Option<PathBuf>,
}

pub fn parse_args() -> Result<Args, lexopt::Error> {
    use lexopt::prelude::*;

    let mut file = None;
    let mut parser = lexopt::Parser::from_env();

    while let Some(arg) = parser.next()? {
        match arg {
            Value(val) if file.is_none() => {
                file = Some(PathBuf::from(val));
            }
            _ => return Err(arg.unexpected()),
        }
    }

    Ok(Args { file })
}
