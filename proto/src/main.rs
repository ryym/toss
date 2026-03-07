mod app;
mod document;
mod line;
mod screen;
mod screen_state;
mod scroll;

use std::env;
use std::path::Path;
use std::process;

use document::Document;
use screen::TermScreen;
use app::App;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: toss-proto <file>");
        process::exit(1);
    }

    let path = Path::new(&args[1]);
    let doc = match Document::from_file(path) {
        Ok(doc) => doc,
        Err(e) => {
            eprintln!("Error reading {}: {e}", path.display());
            process::exit(1);
        }
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
