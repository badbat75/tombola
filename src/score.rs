// tombola/src/score.rs
// This module handles the scorecard logic and prize checking for the Tombola game.

use crate::defs::{BOARDCONFIG, NUMBERSPERCARD, Number};
use serde::{Deserialize, Serialize};

use std::collections::HashMap;

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct ScoreCard {
    pub scorecard: Number,
    pub score_map: HashMap<Number, Vec<String>>, // score_idx -> Vec<cardid>
}

impl ScoreCard {
    pub fn new() -> Self {
        ScoreCard {
            scorecard: 0,
            score_map: HashMap::new(),
        }
    }

    pub fn get_scoremap(&self) -> &HashMap<Number, Vec<String>> {
        &self.score_map
    }

    pub fn with_score(score: Number) -> Self {
        ScoreCard { scorecard: score, score_map: HashMap::new() }
    }

    pub fn update_scorecard(&mut self, score: Number) {
        self.scorecard = score;
    }

    pub fn update_score_map(&mut self, score_idx: Number, card_ids: Vec<String>) {
        self.score_map.insert(score_idx, card_ids);
    }

    pub fn allcards_calculate_score(&self, board_numbers: &[Number], card_assignments: &std::collections::HashMap<String, crate::card::CardAssignment>) -> (Number, Vec<(String, Vec<Number>)>) {
        let mut card_details = Vec::new();
        let current_scorecard = self.scorecard; // Get the current scorecard value
        let mut global_score = 0;
        
        // Iterate through all card assignments
        for (card_id, assignment) in card_assignments {
            // Extract all numbers from the card
            let mut card_numbers = Vec::new();
            for row in &assignment.card_data {
                for &number in row.iter().flatten() {
                    card_numbers.push(number);
                }
            }
            
            // Calculate how many card numbers have been extracted
            let mut extracted_card_numbers = Vec::new();
            for &card_number in &card_numbers {
                if board_numbers.contains(&card_number) {
                    extracted_card_numbers.push(card_number);
                }
            }
            
            // Calculate the score for this specific card
            let card_score = if extracted_card_numbers.len() == card_numbers.len() && !card_numbers.is_empty() {
                // Full card (BINGO)
                NUMBERSPERCARD
            } else if !extracted_card_numbers.is_empty() {
                // Check for line completions within this card
                let mut max_line_score = 0;
                
                // Check each row in the card (3 rows)
                for row_index in 0..3 {
                    let row = &assignment.card_data[row_index];
                    let mut row_extracted_count = 0;
                    
                    for &number in row.iter().flatten() {
                        if board_numbers.contains(&number) {
                            row_extracted_count += 1;
                        }
                    }
                    
                    // Update max line score if this row has more extracted numbers
                    if row_extracted_count > max_line_score {
                        max_line_score = row_extracted_count;
                    }
                }
                
                // Return the highest line score (2, 3, 4, or 5)
                if max_line_score >= 2 {
                    max_line_score
                } else {
                    0
                }
            } else {
                0
            };
            
            // Update the global score to the highest score achieved by any card
            if card_score > global_score {
                global_score = card_score;
            }
            
            // Only include cards that have achieved a meaningful score (>= 2) and have extracted numbers
            if card_score >= 2 && !extracted_card_numbers.is_empty() {
                card_details.push((card_id.clone(), extracted_card_numbers));
            }
        }
        
        // Only return results if the global score is greater than the current scorecard
        if global_score > current_scorecard {
            // Filter card_details to only include cards that achieved the global score
            let filtered_card_details: Vec<(String, Vec<Number>)> = card_details.into_iter()
                .filter(|(card_id, _)| {
                    // Recalculate the score for this card to check if it matches the global score
                    if let Some(assignment) = card_assignments.get(card_id) {
                        let mut card_numbers = Vec::new();
                        for row in &assignment.card_data {
                            for &number in row.iter().flatten() {
                                card_numbers.push(number);
                            }
                        }
                        
                        let mut extracted_card_numbers = Vec::new();
                        for &card_number in &card_numbers {
                            if board_numbers.contains(&card_number) {
                                extracted_card_numbers.push(card_number);
                            }
                        }
                        
                        let card_score = if extracted_card_numbers.len() == card_numbers.len() && !card_numbers.is_empty() {
                            NUMBERSPERCARD
                        } else if !extracted_card_numbers.is_empty() {
                            let mut max_line_score = 0;
                            for row_index in 0..3 {
                                let row = &assignment.card_data[row_index];
                                let mut row_extracted_count = 0;
                                for &number in row.iter().flatten() {
                                    if board_numbers.contains(&number) {
                                        row_extracted_count += 1;
                                    }
                                }
                                if row_extracted_count > max_line_score {
                                    max_line_score = row_extracted_count;
                                }
                            }
                            if max_line_score >= 2 { max_line_score } else { 0 }
                        } else {
                            0
                        };
                        
                        card_score == global_score
                    } else {
                        false
                    }
                })
                .collect();
            
            (global_score, filtered_card_details)
        } else {
            (0, Vec::new())
        }
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
