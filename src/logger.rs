use std::{
    backtrace::{Backtrace, BacktraceStatus},
    env,
    fs::File,
    io::{BufWriter, Write},
    panic,
    sync::{Arc, Mutex},
};

use log::{Log, Metadata};

use crate::error::AnyError;

type SharedFile = Arc<Mutex<BufWriter<File>>>;

/// A minimal logger that writes logs to a file.
/// We cannot use stdout since it is heavily used by the application and cannot capture logs.
pub(crate) struct FileLogger {
    file: SharedFile,
}

impl FileLogger {
    fn new(file: SharedFile) -> Self {
        Self { file }
    }
}

impl Log for FileLogger {
    fn enabled(&self, _: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &log::Record) {
        let mut file = self.file.lock().unwrap();
        writeln!(*file, "[{}] {}", record.level(), record.args()).unwrap();
    }

    fn flush(&self) {
        // never called
    }
}

/// A guard object to flush logs written by [`FileLogger`] on drop.
pub(crate) struct FileLoggerGuard {
    file: SharedFile,
}

impl Drop for FileLoggerGuard {
    fn drop(&mut self) {
        if let Ok(mut file) = self.file.lock() {
            file.flush().ok();
        }
    }
}

/// Set up a [`FileLogger`] as a global logger.
pub(crate) fn setup_file_logger() -> Result<Option<FileLoggerGuard>, AnyError> {
    let env_var = match env::var("RUST_LOG").ok() {
        None => {
            store_logs_on_panic(None);
            return Ok(None);
        }
        Some(var) => var,
    };
    // For now, enable the logger if the environment variable exists regardless of its value.
    if env_var.is_empty() {
        return Ok(None);
    }

    let log_file = File::create("toss-debug.log")?;
    let log_file_handle = Arc::new(Mutex::new(BufWriter::new(log_file)));

    // Set a global logger.
    let logger = FileLogger::new(log_file_handle.clone());
    log::set_boxed_logger(Box::new(logger))?;
    log::set_max_level(log::LevelFilter::Info);

    store_logs_on_panic(Some(log_file_handle.clone()));

    // Sinc the global logger becomes static and never drops, we manually need to flush logs.
    // This guard object handles that.
    Ok(Some(FileLoggerGuard {
        file: log_file_handle.clone(),
    }))
}

fn store_logs_on_panic(log_file_handle: Option<SharedFile>) {
    // On a panic, try to store information in a file instead of stdout.
    // This is because stdout is heavily used by the application and cannot capture logs.
    let original_hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        // Flush logs.
        if let Some(handle) = &log_file_handle
            && let Ok(mut file) = handle.lock()
        {
            file.flush().ok();
        }

        if let Ok(mut panic_log_file) = File::create("toss-panic.log") {
            let backtrace = Backtrace::capture();
            let backtrace = match backtrace.status() {
                BacktraceStatus::Captured => backtrace.to_string(),
                _ => String::new(),
            };
            let info = format!("{panic_info}\n{backtrace}");
            panic_log_file.write_all(info.as_bytes()).ok();
        }

        original_hook(panic_info);
    }));
}
