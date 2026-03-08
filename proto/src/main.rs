mod app;
mod document;
mod line;
#[cfg(test)]
mod mock_screen;
mod screen;
mod screen_state;
mod scroll;

#[cfg(test)]
mod app_tests;

use std::io::{self, IsTerminal};
use std::path::Path;
use std::process;

use app::App;
use document::Document;
use screen::TermScreen;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    let doc = if !args.is_empty() {
        let path = Path::new(&args[0]);
        match Document::from_file(path) {
            Ok(doc) => doc,
            Err(e) => {
                eprintln!("Error reading {}: {e}", path.display());
                process::exit(1);
            }
        }
    } else if !io::stdin().is_terminal() {
        match Document::from_reader(&mut io::stdin().lock()) {
            Ok(doc) => doc,
            Err(e) => {
                eprintln!("Error reading stdin: {e}");
                process::exit(1);
            }
        }
    } else {
        eprintln!("Usage: toss-proto <file>");
        eprintln!("       command | toss-proto");
        process::exit(1);
    };

    let screen = match TermScreen::new() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error initializing terminal: {e}");
            process::exit(1);
        }
    };

    let mut app = match App::new(screen, doc) {
        Ok(app) => app,
        Err(e) => {
            eprintln!("Error starting app: {e}");
            process::exit(1);
        }
    };

    if let Err(e) = app.run() {
        eprintln!("Error: {e}");
        process::exit(1);
    }
}
