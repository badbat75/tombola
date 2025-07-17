// src/main.rs
// This is the main entry point for the Tombola game.

use tombola::server;
use tombola::config::ServerConfig;

// Function to wait for a key press and return true if ESC is pressed, false otherwise
#[tokio::main]
async fn main() {
    // Load server configuration
    let config = ServerConfig::load_or_default();
    
    // Start the API server with all components created internally
    let (server_handle, _shutdown_signal) = server::start_server(config.clone());

    println!("🎯 Tombola Game Server Started");
    println!("📡 API Server running on http://{}:{}", config.host, config.port);
    println!("🚪 Press Ctrl+C to stop the server");

    // Simple main loop - wait for the server to finish or Ctrl+C
    match server_handle.await {
        Ok(_) => println!("✅ API server stopped successfully."),
        Err(e) => eprintln!("❌ Error waiting for server shutdown: {e:?}"),
    }
}