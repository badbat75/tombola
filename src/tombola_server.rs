// src/main.rs
// This is the main entry point for the Tombola game.

use tombola::server;
use tombola::config::ServerConfig;
use tombola::logging::{init_logging, log, LogLevel};

const MODULE_NAME: &str = "tombola_server";

async fn shutdown_signal() {
    match tokio::signal::ctrl_c().await {
        Ok(()) => {
            log(LogLevel::Info, MODULE_NAME, "Received Ctrl+C signal, initiating graceful shutdown...");
            println!("Received Ctrl+C signal, initiating graceful shutdown...");
        }
        Err(err) => {
            log(LogLevel::Error, MODULE_NAME, &format!("Error listening for shutdown signal: {}", err));
            eprintln!("Error listening for shutdown signal: {}", err);
        }
    }
}

// Function to wait for a key press and return true if ESC is pressed, false otherwise
#[tokio::main]
async fn main() {
    // Load server configuration first
    let config = ServerConfig::load_or_default();

    // Initialize the non-blocking logging system with configuration
    init_logging(&config);

    // Start the API server with all components created internally
    let mut server_handle = server::start_server(config.clone());

    println!("📄 Tombola Game Server Started");
    log(LogLevel::Info, MODULE_NAME, "Tombola Game Server Started");
    println!("API Server running on http://{}:{}", config.host, config.port);
    log(LogLevel::Info, MODULE_NAME, &format!("API Server running on http://{}:{}", config.host, config.port));
    println!("Press Ctrl+C to stop the server");

    // Wait for shutdown signal or server to finish
    tokio::select! {
        _ = shutdown_signal() => {
            log(LogLevel::Info, MODULE_NAME, "Graceful shutdown initiated");
            // Cancel the server task
            server_handle.abort();
            // Wait a bit for cleanup
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

            // Check if server finished cleanly
            match server_handle.await {
                Ok(()) => log(LogLevel::Info, MODULE_NAME, "API server stopped successfully after abort."),
                Err(e) if e.is_cancelled() => log(LogLevel::Info, MODULE_NAME, "API server was cancelled for graceful shutdown"),
                Err(e) => log(LogLevel::Warning, MODULE_NAME, &format!("Server cleanup warning: {e:?}")),
            }
        }
        result = &mut server_handle => {
            match result {
                Ok(()) => log(LogLevel::Info, MODULE_NAME, "API server stopped successfully."),
                Err(e) => log(LogLevel::Error, MODULE_NAME, &format!("Error waiting for server shutdown: {e:?}")),
            }
        }
    }
    println!("Application shutdown complete");
    log(LogLevel::Info, MODULE_NAME, "Application shutdown complete");
}
