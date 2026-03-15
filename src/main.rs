mod ansi;
mod app;
mod cli;
mod document;
mod line;
mod line_editor;
mod logger;
mod page;
mod screen;
mod scroll;
mod search;
mod status_line;
mod viewport;

#[cfg(test)]
mod tests;

use std::io::{self, IsTerminal};
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

    let args = match cli::parse_args() {
        Ok(cli::Action::PrintVersion) => {
            println!("toss {}", cli::version());
            return 0;
        }
        Ok(cli::Action::Run(args)) => args,
        Err(e) => {
            eprintln!("Error: {e}");
            return 1;
        }
    };

    let doc = if let Some(path) = &args.file {
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
