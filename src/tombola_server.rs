// src/main.rs
// This is the main entry point for the Tombola game.

use std::sync::{Arc, Mutex};

use tombola::defs::{Number, NUMBERSPERCARD};
use tombola::pouch::Pouch;
use tombola::board::Board;
use tombola::{terminal, server};

enum IOList {
    Terminal,
}

// Default program input/output:
const IO: IOList = IOList::Terminal;

fn next_extraction (iodevice: &IOList) -> bool {
    match iodevice {
        IOList::Terminal => { terminal::hitkey() }
    }
}

fn show_on(iodevice: &IOList, board: &Board, pouch: &[Number]) {
    match iodevice {
        IOList::Terminal => { terminal::show_on_terminal(board, pouch) }
    }
}

// Function to wait for a key press and return true if ESC is pressed, false otherwise
#[tokio::main]
async fn main() {
    // Initialize and fill the pouch
    let pouch_ref = Arc::new(Mutex::new(Pouch::new()));

    // Create shared reference to the board (single source of truth)
    let board_ref = Arc::new(Mutex::new(Board::new()));
    
    // Start the API server in the background with the board reference
    let (server_handle, shutdown_signal, card_manager) = server::start_server(Arc::clone(&board_ref),Arc::clone(&pouch_ref));

    loop {
        // Check if pouch is empty
        let is_empty = if let Ok(pouch) = pouch_ref.lock() {
            pouch.is_empty()
        } else {
            break; // Exit if we can't acquire the lock
        };
        
        if is_empty {
            break;
        }
        
        // Expect event for next extraction
        if next_extraction(&IO) {
            break;
        }
        
        // Randomly extract a number index from the pouch
        let extracted: Number = if let Ok(mut pouch) = pouch_ref.lock() {
            pouch.extract()
        } else {
            break; // Exit if we can't acquire the lock
        };

        // Lock the shared board and perform all operations
        if let Ok(mut board) = board_ref.lock() {
            // Add the extracted number to the shared board and check for prizes
            board.push(extracted);

            ////////////////////////////////////////////////////////////

            // Calculate score and numbers to mark
            let (new_score, numbers_to_mark) = board.get_scorecard_ref().board_calculate_score(&board.get_numbers());
            // Update the scorecard score
            board.update_scorecard(new_score);
            
            // Update marked numbers based on scoring
            board.update_marked_numbers(numbers_to_mark);

            ////////////////////////////////////////////////////////////

            // Calculate scores for all cards
            if let Ok(card_assignments_map) = card_manager.lock() {
                let assignments = card_assignments_map.get_all_assignments();
                
                let all_card_scores = board.get_scorecard_ref().allcards_calculate_score(
                    &board.get_numbers(), 
                    assignments
                );
                
                if !all_card_scores.is_empty() {
                    println!("\n🎯 All Card Scores:");
                    for (card_id, score, marked_numbers) in all_card_scores {
                        println!("  Card {}: Score = {}, Marked = {:?}", card_id, score, marked_numbers);
                    }
                    println!();
                }
            }

            ////////////////////////////////////////////////////////////

            // Show the current state on configured IO device
            if let Ok(pouch) = pouch_ref.lock() {
                show_on(&IO, &board, &pouch.numbers);
            }
            
            // If the scorecard reaches the number of numbers per card, break the loop
            if board.get_scorecard() == NUMBERSPERCARD { break }
        }
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