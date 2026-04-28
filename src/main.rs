use std::process;

fn main() {
    match toss::run() {
        Ok(()) => {}
        Err(e) => {
            eprintln!("{e}");
            process::exit(e.exit_code);
        }
    }
}
