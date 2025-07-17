// src/extraction.rs
// Core extraction logic shared between tombola_server and API server

use std::sync::{Arc, Mutex};
use crate::defs::Number;
use crate::pouch::Pouch;
use crate::board::Board;
use crate::score::ScoreCard;
use crate::card::CardAssignmentManager;

// Core extraction function that handles the game logic
pub fn perform_extraction(
    pouch_ref: &Arc<Mutex<Pouch>>,
    board_ref: &Arc<Mutex<Board>>,
    scorecard_ref: &Arc<Mutex<ScoreCard>>,
    card_manager: &Arc<Mutex<CardAssignmentManager>>,
    current_working_score: Number,
) -> Result<(Number, Number), String> {
    // Open pouch mutex once for empty check and extraction
    let extracted: Number = {
        let mut pouch = match pouch_ref.lock() {
            Ok(p) => p,
            Err(_) => return Err("Failed to acquire pouch lock".to_string()),
        };
        if pouch.is_empty() {
            return Err("Pouch is empty".to_string());
        }
        pouch.extract()
    };
    
    // Check if extraction was successful (pouch not empty)
    if extracted == 0 {
        return Err("No numbers remaining in pouch".to_string());
    }
    
    // Perform all board operations in a coordinated manner
    {
        // Keep board and scorecard locks open for the entire operation
        if let Ok(mut board) = board_ref.lock() {
            if let Ok(scorecard) = scorecard_ref.lock() {
                // Add the extracted number to the board (includes scoring and marking)
                board.push(extracted, &scorecard);
            } else {
                return Err("Failed to acquire scorecard lock".to_string());
            }
        } else {
            return Err("Failed to acquire board lock".to_string());
        }
    }; // Both locks are released here

    // Calculate scores for all cards - keep all needed locks open
    let new_working_score = {
        // Calculate and update the best score using ScoreCard method with all locks coordinated
        if let Ok(card_assignments_manager) = card_manager.lock() {
            if let Ok(mut scorecard) = scorecard_ref.lock() {
                if let Ok(board) = board_ref.lock() {
                    scorecard.calculate_and_update_best_score(&board, &card_assignments_manager, current_working_score)
                } else {
                    return Err("Failed to acquire board lock for scoring".to_string());
                }
            } else {
                return Err("Failed to acquire scorecard lock for scoring".to_string());
            }
        } else {
            return Err("Failed to acquire card manager lock".to_string());
        }
    };

    Ok((extracted, new_working_score))
}
