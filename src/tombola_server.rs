// src/main.rs
// This is the main entry point for the Tombola game.

use std::sync::{Arc, Mutex};

use tombola::defs::{Number, NUMBERSPERCARD};
use tombola::pouch::Pouch;
use tombola::board::Board;
use tombola::score::ScoreCard;
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

fn show_on(iodevice: &IOList, board: &Board, pouch: &[Number], scorecard: &ScoreCard) {
    match iodevice {
        IOList::Terminal => { terminal::show_on_terminal(board, pouch, scorecard) }
    }
}

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
    let (server_handle, shutdown_signal, card_manager) = server::start_server(Arc::clone(&board_ref), Arc::clone(&pouch_ref), Arc::clone(&scorecard_ref));

    // Wait before starting the game loop
    if next_extraction(&IO) {
        return;
    }
    let mut current_score = 1;

    loop {
        // Open pouch mutex once for empty check and extraction
        let extracted: Number = {
            let mut pouch = match pouch_ref.lock() {
                Ok(p) => p,
                Err(_) => break, // Exit if we can't acquire the lock
            };
            if pouch.is_empty() {
                break;
            }
            pouch.extract()
        };
        
        // Perform all board and scorecard operations in a coordinated manner
        let boardscore_value = {
            // Keep board and scorecard locks open for the entire operation
            if let Ok(mut board) = board_ref.lock() {
                if let Ok(scorecard) = scorecard_ref.lock() {
                    // Add the extracted number to the board
                    board.push(extracted);

                    // Get board numbers reference for calculations
                    let board_numbers = board.get_numbers();

                    // Calculate score and numbers to mark
                    let (score, numbers_to_mark) = scorecard.board_calculate_score(board_numbers);

                    // Update marked numbers based on scoring
                    board.update_marked_numbers(numbers_to_mark);

                    score
                } else {
                    break; // Exit if we can't acquire scorecard lock
                }
            } else {
                break; // Exit if we can't acquire board lock
            }
        }; // Both locks are released here
 
        ////////////////////////////////////////////////////////////

        // Calculate scores for all cards - keep all needed locks open
        let mut allcardscore_value = 0;
        let mut card_details = Vec::new();
        if let Ok(card_assignments_map) = card_manager.lock() {
            let assignments = card_assignments_map.get_all_assignments();

            let result = {
                if let Ok(scorecard) = scorecard_ref.lock() {
                    if let Ok(board) = board_ref.lock() {
                        scorecard.allcards_calculate_score(board.get_numbers(), assignments)
                    } else {
                        (0, Vec::new())
                    }
                } else {
                    (0, Vec::new())
                }
            }; // Scorecard and board locks are released here

            allcardscore_value = result.0;
            card_details = result.1;
        } // Card manager lock is released here

        let bestscore = std::cmp::max(allcardscore_value, boardscore_value);
        // If the best score is greater than the current score, update the scorecard
        if bestscore > current_score { 
            if let Ok(mut scorecard) = scorecard_ref.lock() {
                // Assign the card_id list
                let card_ids = if allcardscore_value >= boardscore_value {
                    // Add score_idx and card_id list to score_map
                    let mut card_ids: Vec<String> = card_details.iter().map(|(card_id, _)| card_id.clone()).collect();
                    if allcardscore_value == boardscore_value {
                        card_ids.push("0000000000000000".to_string());
                    }
                    card_ids
                } else {
                    let mut card_ids: Vec<String> = Vec::new();
                    card_ids.push("0000000000000000".to_string());
                    card_ids
                };
                scorecard.update_scorecard(bestscore);
                scorecard.update_score_map(bestscore, card_ids);    
            }
            current_score = bestscore;
        }

        ////////////////////////////////////////////////////////////

        // Show the current state on configured IO device - keep both locks open
        {
            if let Ok(pouch) = pouch_ref.lock() {
                if let Ok(board) = board_ref.lock() {
                    if let Ok(scorecard) = scorecard_ref.lock() {
                    show_on(&IO, &board, &pouch.numbers, &scorecard);
                    } // Scorecard lock released here
                } // Board lock released here
            } // Pouch lock released here
        }

        // Expect event for next extraction
        if next_extraction(&IO) {
            break;
        }

        // If the scorecard reaches the number of numbers per card, break the loop
        if current_score == NUMBERSPERCARD { break }
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