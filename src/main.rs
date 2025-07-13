// src/main.rs
// This is the main entry point for the Tombola game.

use std::sync::{Arc, Mutex};

mod defs;
use defs::{Number, FIRSTNUMBER, LASTNUMBER, NUMBERSPERCARD};
mod board;
use board::Board;
mod terminal;
mod server;

#[derive(Copy, Clone)]
enum IOList {
    Terminal,
}

// Default program input/output:
const IO: IOList = IOList::Terminal;

fn next_extraction (iodevice: IOList) -> bool {
    match iodevice {
        IOList::Terminal => { terminal::hitkey() }
    }
}

fn show_on(iodevice: IOList, board: &Board, pouch: &[Number], extracted: Number, scorecard: &mut Number, itemsleft: usize) {
    match iodevice {
        IOList::Terminal => { terminal::show_on_terminal(board, pouch, extracted, scorecard, itemsleft) }
    }
}

// Function to wait for a key press and return true if ESC is pressed, false otherwise
#[tokio::main]
async fn main() {
    let mut pouch: Vec<Number> = (FIRSTNUMBER..=LASTNUMBER).collect();
    let mut itemsleft = pouch.len();
    let mut scorecard = 0;

    // Create a shared reference to the board (single source of truth)
    let board_ref = Arc::new(Mutex::new(Board::new()));
    
    // Start the API server in the background with the board reference
    let (server_handle, shutdown_signal) = server::start_server(Arc::clone(&board_ref));

    while ! pouch.is_empty() {
        // Expect event for next extraction
        if next_extraction(IO) {
            break;
        }
        // Randomly extract a number index from the pouch
        let random_index = rand::random_range(0..itemsleft);
        let extracted: Number = pouch.remove(random_index);
        
        itemsleft = pouch.len();
        
        // Lock the shared board once and perform all operations
        if let Ok(mut board) = board_ref.lock() {
            // Add the extracted number to the shared board and check for prizes
            board.push(extracted, &mut scorecard);
            
            // Show the current state on configured IO device
            show_on(IO, &board, &pouch, extracted, &mut scorecard, itemsleft);
        }

        // If the scorecard reaches the number of numbers per card, break the loop
        if scorecard == NUMBERSPERCARD { break }
    }
    
    // Signal the server to shutdown
    shutdown_signal.store(true, std::sync::atomic::Ordering::Relaxed);
    println!("Game ended. Shutting down API server...");
    
    // Wait for the server thread to finish
    if let Err(e) = server_handle.await {
        eprintln!("Error waiting for server shutdown: {e:?}");
    }
    
    println!("API server stopped successfully.");
}