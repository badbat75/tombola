// tombola/src/score.rs
// This module handles the scorecard logic and prize checking for the Tombola game.

use crate::defs::{BOARDCONFIG, NUMBERSPERCARD, Number};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct ScoreCard {
    scorecard: Number,
}

impl ScoreCard {
    pub fn new() -> Self {
        ScoreCard {
            scorecard: 0,
        }
    }
    
    pub fn get_scorecard(&self) -> Number {
        self.scorecard
    }
    
    pub fn with_score(score: Number) -> Self {
        ScoreCard { scorecard: score }
    }

    pub fn board_calculate_score(&self, board_numbers: &[Number]) -> (Number, Vec<Number>) {
        // Calculate score based on the last extracted number
        if let Some(&last_number) = board_numbers.last() {
            self.board_score_check(board_numbers, last_number)
        } else {
            (0, Vec::new())
        }
    }
    
    pub fn board_score_check(&self, board_numbers: &[Number], extracted: Number) -> (Number, Vec<Number>) {
        // Calculate score based on the last extracted number
        let numbers_per_row = (BOARDCONFIG.cols_per_card * BOARDCONFIG.cards_per_row) as i8;
        
        // Store the previous scorecard value BEFORE any updates
        let previous_scorecard = self.scorecard;
        let mut new_scorecard = 0;
        
        // Calculate extracted number's position for line checking
        let extracted_ypos = (extracted as i8 - 1) / numbers_per_row + 1;
        let extracted_xpos = (extracted as i8 - 1) % numbers_per_row;
        let extracted_card = extracted_xpos / BOARDCONFIG.cols_per_card as i8;
        
        // Check for complete cards (15 numbers) first
        let total_cards = (BOARDCONFIG.cards_per_row * BOARDCONFIG.cards_per_col) as i8;
        
        let mut bingo_found = false;
        for card_index in 0..total_cards {
            let card_row = card_index / BOARDCONFIG.cards_per_row as i8;
            let card_col = card_index % BOARDCONFIG.cards_per_row as i8;
            
            let mut card_numbers_found = 0;
            let mut card_numbers: Vec<Number> = Vec::new();
            
            // Check all numbers in this card
            for row in 0..BOARDCONFIG.rows_per_card as i8 {
                for col in 0..BOARDCONFIG.cols_per_card as i8 {
                    let number = 1 + (card_row * BOARDCONFIG.rows_per_card as i8 + row) * numbers_per_row + 
                               card_col * BOARDCONFIG.cols_per_card as i8 + col;
                    
                    card_numbers.push(number as Number);
                    
                    if board_numbers.contains(&(number as Number)) {
                        card_numbers_found += 1;
                    }
                }
            }
            
            // If this card is complete (15 numbers), set scorecard to 15
            if card_numbers_found == NUMBERSPERCARD {
                new_scorecard = NUMBERSPERCARD;
                bingo_found = true;
                break;
            }
        }
        
        // Only do line scoring if no BINGO was found
        if !bingo_found {
            // Count numbers in the same row and card as the extracted number (for line scoring)
            let mut same_row_card_count = 0;
            
            for &num in board_numbers {
                if num == extracted {
                    continue; // Skip the extracted number itself for counting
                }
                
                let num_ypos = (num as i8 - 1) / numbers_per_row + 1;
                let num_xpos = (num as i8 - 1) % numbers_per_row;
                let num_card = num_xpos / BOARDCONFIG.cols_per_card as i8;
                
                // Check if same row and same card
                if num_ypos == extracted_ypos && num_card == extracted_card {
                    same_row_card_count += 1;
                }
            }
            
            // Update scorecard for line scoring
            // Only update if we haven't achieved this goal before
            let current_line_score = same_row_card_count + 1;
            if current_line_score > previous_scorecard { 
                new_scorecard = current_line_score;
            } else {
                new_scorecard = previous_scorecard;
            }
        }

        // Return numbers to mark based on scorecard achievement
        // Only return numbers if we just achieved a NEW score (higher than previous)
        let is_new_achievement = new_scorecard > previous_scorecard;
        
        let numbers_to_mark = if is_new_achievement {
            match new_scorecard {
                2..=5 => {
                    // Return numbers that are part of the current scoring line
                    let mut numbers_to_mark = Vec::new();
                    for &num in board_numbers {
                        let num_ypos = (num as i8 - 1) / numbers_per_row + 1;
                        let num_xpos = (num as i8 - 1) % numbers_per_row;
                        let num_card = num_xpos / BOARDCONFIG.cols_per_card as i8;
                        
                        // Only mark if it's in the same row and card as the extracted number
                        if num_ypos == extracted_ypos && num_card == extracted_card {
                            numbers_to_mark.push(num);
                        }
                    }
                    numbers_to_mark
                },
                x if x == NUMBERSPERCARD => {
                    // Return all numbers in the completed card
                    for card_index in 0..total_cards {
                        let card_row = card_index / BOARDCONFIG.cards_per_row as i8;
                        let card_col = card_index % BOARDCONFIG.cards_per_row as i8;
                        
                        let mut card_numbers_found = 0;
                        let mut card_numbers: Vec<Number> = Vec::new();
                        
                        // Collect all numbers in this card (regardless of extraction)
                        for row in 0..BOARDCONFIG.rows_per_card as i8 {
                            for col in 0..BOARDCONFIG.cols_per_card as i8 {
                                let number = 1 + (card_row * BOARDCONFIG.rows_per_card as i8 + row) * numbers_per_row + 
                                           card_col * BOARDCONFIG.cols_per_card as i8 + col;
                                
                                card_numbers.push(number as Number);
                                
                                if board_numbers.contains(&(number as Number)) {
                                    card_numbers_found += 1;
                                }
                            }
                        }
                        
                        // If this card is complete, return all its extracted numbers
                        if card_numbers_found == NUMBERSPERCARD {
                            return (new_scorecard, board_numbers.iter()
                                .filter(|&num| card_numbers.contains(num))
                                .cloned()
                                .collect());
                        }
                    }
                    Vec::new()
                },
                _ => Vec::new()
            }
        } else {
            Vec::new()
        };
        
        (new_scorecard, numbers_to_mark)
    }
}
