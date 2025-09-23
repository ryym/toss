use std::io::Write;
use std::panic;

use toss::app;

fn main() {
    let original_hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        let message = if let Some(s) = panic_info.payload().downcast_ref::<String>() {
            s.as_str()
        } else if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
            s
        } else {
            "Unknown panic message"
        };
        let info = format!("{}\n{:?}", message, panic_info);
        let mut log_file = std::fs::File::create("toss-panic.log").unwrap();
        let _ = log_file.write_all(info.as_bytes());
        original_hook(panic_info);
    }));

    let result = app::run();
    println!("result {:?}", result);
}
