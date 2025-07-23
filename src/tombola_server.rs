// src/main.rs
// This is the main entry point for the Tombola game.

use tombola::server;
use tombola::config::ServerConfig;
use tombola::logging::{log_info, log_error};

// Function to wait for a key press and return true if ESC is pressed, false otherwise
#[tokio::main]
async fn main() {
    // Load server configuration
    let config = ServerConfig::load_or_default();

    // Start the API server with all components created internally
    let (server_handle, _shutdown_signal) = server::start_server(config.clone());

    log_info("Tombola Game Server Started");
    log_info(&format!("API Server running on http://{}:{}", config.host, config.port));
    log_info("Press Ctrl+C to stop the server");

    // Simple main loop - wait for the server to finish or Ctrl+C
    match server_handle.await {
        Ok(()) => log_info("API server stopped successfully."),
        Err(e) => log_error(&format!("Error waiting for server shutdown: {e:?}")),
    }
}
