// logging.rs
// Enhanced logging utility for tombola server with file support

use chrono::Local;
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};
use tokio::sync::mpsc;

use crate::config::{LoggingMode, ServerConfig};

/// Log level enum
#[derive(Debug, Clone, Copy)]
pub enum LogLevel {
    Debug,
    Info,
    Warning,
    Error,
}

impl LogLevel {
    fn as_str(&self) -> &'static str {
        match self {
            LogLevel::Debug => "DEBUG",
            LogLevel::Info => "INFO",
            LogLevel::Warning => "WARNING",
            LogLevel::Error => "ERROR",
        }
    }
}

/// Log message structure
#[derive(Debug)]
struct LogMessage {
    level: LogLevel,
    module: String,
    message: String,
    timestamp: String,
}

/// Logging configuration
#[derive(Debug, Clone)]
struct LoggingConfig {
    mode: LoggingMode,
    logpath: String,
}

/// File writer cache for module-specific log files
type FileWriterCache = Arc<Mutex<HashMap<String, Arc<Mutex<File>>>>>;

/// Global sender for log messages and configuration
static LOG_SENDER: OnceLock<mpsc::UnboundedSender<LogMessage>> = OnceLock::new();
static LOGGING_CONFIG: OnceLock<LoggingConfig> = OnceLock::new();

/// Initialize the logging system with configuration - should be called once at startup
pub fn init_logging(config: &ServerConfig) {
    let logging_config = LoggingConfig {
        mode: config.logging.clone(),
        logpath: config.logpath.clone(),
    };

    // Store the configuration globally
    if LOGGING_CONFIG.set(logging_config.clone()).is_err() {
        eprintln!("Warning: Logging configuration already initialized");
        return;
    }

    let (sender, mut receiver) = mpsc::unbounded_channel::<LogMessage>();

    // Store the sender globally
    if LOG_SENDER.set(sender).is_err() {
        eprintln!("Warning: Logging system already initialized");
        return;
    }

    // Create log directory if using file logging
    if matches!(logging_config.mode, LoggingMode::File | LoggingMode::Both) {
        if let Err(e) = std::fs::create_dir_all(&logging_config.logpath) {
            eprintln!("Failed to create log directory '{}': {}", logging_config.logpath, e);
            return;
        }
    }

    // Initialize file writer cache for module-specific logs
    let file_writers: FileWriterCache = Arc::new(Mutex::new(HashMap::new()));

    // Spawn background task to handle log messages
    tokio::spawn(async move {
        while let Some(log_msg) = receiver.recv().await {
            let formatted_message = format!("{} - {} - [{}] {}",
                log_msg.timestamp,
                log_msg.level.as_str(),
                log_msg.module,
                log_msg.message
            );

            // Handle console output
            if matches!(logging_config.mode, LoggingMode::Console | LoggingMode::Both) {
                match log_msg.level {
                    LogLevel::Error => eprintln!("{formatted_message}"),
                    _ => println!("{formatted_message}"),
                }
            }

            // Handle file output
            if matches!(logging_config.mode, LoggingMode::File | LoggingMode::Both) {
                if let Err(e) = write_to_file(&file_writers, &logging_config, &log_msg, &formatted_message).await {
                    eprintln!("Failed to write to log file: {}", e);
                }
            }
        }
    });
}

/// Write log message to module-specific file
async fn write_to_file(
    file_writers: &FileWriterCache,
    config: &LoggingConfig,
    log_msg: &LogMessage,
    formatted_message: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let log_filename = format!("{}.log", log_msg.module);
    let log_path = Path::new(&config.logpath).join(log_filename);

    // Get or create file writer for this module
    let file_writer = {
        let mut writers = file_writers.lock().map_err(|_| "Failed to lock file writers")?;

        if let Some(writer) = writers.get(&log_msg.module) {
            writer.clone()
        } else {
            // Create new file writer for this module
            let file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&log_path)
                .map_err(|e| format!("Failed to open log file '{}': {}", log_path.display(), e))?;

            let writer = Arc::new(Mutex::new(file));
            writers.insert(log_msg.module.clone(), writer.clone());
            writer
        }
    };

    // Write to file
    let mut file = file_writer.lock().map_err(|_| "Failed to lock file writer")?;
    writeln!(file, "{}", formatted_message)
        .map_err(|e| format!("Failed to write to log file: {}", e))?;
    file.flush()
        .map_err(|e| format!("Failed to flush log file: {}", e))?;

    Ok(())
}

/// Core logging function - non-blocking
pub fn log(level: LogLevel, module: &str, message: &str) {
    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

    let log_msg = LogMessage {
        level,
        module: module.to_string(),
        message: message.to_string(),
        timestamp,
    };

    // Try to send the message to the background thread
    if let Some(sender) = LOG_SENDER.get() {
        if sender.send(log_msg).is_err() {
            // Fallback to direct printing if channel is closed
            let formatted_message = format!("{} - {} - [{}] {}",
                Local::now().format("%Y-%m-%d %H:%M:%S"),
                level.as_str(),
                module,
                message
            );
            match level {
                LogLevel::Error => eprintln!("{formatted_message}"),
                _ => println!("{formatted_message}"),
            }
        }
    } else {
        // Fallback to direct printing if logging not initialized
        let formatted_message = format!("{} - {} - [{}] {}",
            Local::now().format("%Y-%m-%d %H:%M:%S"),
            level.as_str(),
            module,
            message
        );
        match level {
            LogLevel::Error => eprintln!("{formatted_message}"),
            _ => println!("{formatted_message}"),
        }
    }
}
