// logging.rs
// Simple logging utility for tombola server

use chrono::Local;

/// Log level enum
#[derive(Debug, Clone, Copy)]
pub enum LogLevel {
    Info,
    Error,
    Warning,
}

impl LogLevel {
    fn as_str(&self) -> &'static str {
        match self {
            LogLevel::Info => "INFO",
            LogLevel::Error => "ERROR",
            LogLevel::Warning => "WARNING",
        }
    }
}

/// Format and print a log message with timestamp
pub fn log_message(level: LogLevel, message: &str) {
    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S");
    println!("{} - {} - {}", timestamp, level.as_str(), message);
}

/// Log an info message
pub fn log_info(message: &str) {
    log_message(LogLevel::Info, message);
}

/// Log an error message
pub fn log_error(message: &str) {
    log_message(LogLevel::Error, message);
}

/// Log a warning message
pub fn log_warning(message: &str) {
    log_message(LogLevel::Warning, message);
}

/// Format and print an error log message to stderr with timestamp
pub fn log_error_stderr(message: &str) {
    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S");
    eprintln!("{} - {} - {}", timestamp, LogLevel::Error.as_str(), message);
}
