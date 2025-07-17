// src/main.rs
// This is the main entry point for the Tombola game.

use std::sync::{Arc, Mutex};

use tombola::server;
use tombola::pouch::Pouch;
use tombola::board::Board;
use tombola::score::ScoreCard;

// Function to wait for a key press and return true if ESC is pressed, false otherwise
#[tokio::main]
async fn main() {
    // Initialize and fill the pouch
    let pouch_ref = Arc::new(Mutex::new(Pouch::new()));

    // Create shared reference to the board (single source of truth)
    let board_ref = Arc::new(Mutex::new(Board::new()));
    
    // Create a separate scorecard instance
    let scorecard_ref = Arc::new(Mutex::new(ScoreCard::new()));
    
    // Start the API server in the background with the board reference
    let (server_handle, _shutdown_signal, _card_manager) = server::start_server(Arc::clone(&board_ref), Arc::clone(&pouch_ref), Arc::clone(&scorecard_ref));

    println!("🎯 Tombola Game Server Started");
    println!("📡 API Server running on http://127.0.0.1:3000");
    println!("🎮 Use board_client for game display");
    println!("� Use /extract endpoint with board client ID for number extraction");
    println!("🚪 Press Ctrl+C to stop the server");

    // Simple main loop - wait for the server to finish or Ctrl+C
    match server_handle.await {
        Ok(_) => println!("✅ API server stopped successfully."),
        Err(e) => eprintln!("❌ Error waiting for server shutdown: {e:?}"),
    }
}