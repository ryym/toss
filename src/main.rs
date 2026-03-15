mod ansi;
mod app;
mod document;
mod highlight;
mod line;
mod line_editor;
mod logger;
#[cfg(test)]
mod mock_screen;
mod page;
mod screen;
mod scroll;
mod search;
mod status_line;
mod viewport;

#[cfg(test)]
mod tests;

use std::io::{self, IsTerminal};
use std::path::Path;
use std::process;

use app::App;
use document::Document;
use screen::TermScreen;

fn main() {
    let code = run();
    process::exit(code);
}

fn run() -> i32 {
    let _log_guard = match logger::setup_file_logger() {
        Ok(guard) => guard,
        Err(e) => {
            eprintln!("Error setting up logger: {e}");
            return 1;
        }
    };

    let args: Vec<String> = std::env::args().skip(1).collect();
    log::debug!("Args: {args:?}");

    let doc = if !args.is_empty() {
        let path = Path::new(&args[0]);
        log::debug!("Read file: {}", path.display());
        match Document::from_file(path) {
            Ok(doc) => doc,
            Err(e) => {
                eprintln!("Error reading {}: {e}", path.display());
                return 1;
            }
        }
    } else if !io::stdin().is_terminal() {
        log::debug!("Read from stdin");
        match Document::from_reader(&mut io::stdin().lock()) {
            Ok(doc) => doc,
            Err(e) => {
                eprintln!("Error reading stdin: {e}");
                return 1;
            }
        }
    } else {
        eprintln!("Usage: toss <file>");
        eprintln!("       command | toss");
        return 1;
    };

    let screen = match TermScreen::new() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error initializing terminal: {e}");
            return 1;
        }
    };

    let mut app = match App::new(screen, doc) {
        Ok(app) => app,
        Err(e) => {
            eprintln!("Error starting app: {e}");
            return 1;
        }
    };

    if let Err(e) = app.run() {
        eprintln!("Error: {e}");
        return 1;
    }

    0
}
