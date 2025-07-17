// tombola/src/score.rs
// This module handles the scorecard logic and prize checking for the Tombola game.

use crate::defs::{BOARDCONFIG, NUMBERSPERCARD, Number};
use crate::board::Board;
use crate::card::CardAssignmentManager;
use serde::{Deserialize, Serialize};

use std::collections::HashMap;

#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq)]
pub struct ScoreAchievement {
    pub client_id: String,
    pub card_id: String,
    pub numbers: Vec<Number>,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct ScoreCard {
    /// Official recorded achievement level - used for API responses and client display
    pub published_score: Number,
    pub score_map: HashMap<Number, Vec<ScoreAchievement>>, // score_idx -> Vec<ScoreAchievement>
}

impl ScoreCard {
    pub fn new() -> Self {
        ScoreCard {
            published_score: 0,
            score_map: HashMap::new(),
        }
    }

    pub fn get_scorecard(&self) -> Number {
        self.published_score
    }

    pub fn get_scoremap(&self) -> &HashMap<Number, Vec<ScoreAchievement>> {
        &self.score_map
    }

    pub fn with_score(score: Number) -> Self {
        ScoreCard { 
            published_score: score, 
            score_map: HashMap::new() 
        }
    }

    pub fn update_scorecard(&mut self, score: Number) {
        self.published_score = score;
    }

    pub fn update_score_map(&mut self, score_idx: Number, achievements: Vec<ScoreAchievement>) {
        self.score_map.insert(score_idx, achievements);
    }

    pub fn allcards_calculate_score(&self, board_numbers: &[Number], card_assignments: &std::collections::HashMap<String, crate::card::CardAssignment>) -> (Number, Vec<(String, Vec<Number>)>) {
        let mut card_details = Vec::new();
        let current_published_score = self.published_score; // Get the current published score value
        let mut global_score = 0;
        
        // Helper function to calculate score and contributing numbers for a card
        let calculate_card_score = |assignment: &crate::card::CardAssignment| -> (Number, Vec<Number>) {
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
            if extracted_card_numbers.len() == card_numbers.len() && !card_numbers.is_empty() {
                // Full card (BINGO) - all extracted numbers contribute
                (NUMBERSPERCARD, extracted_card_numbers)
            } else if !extracted_card_numbers.is_empty() {
                // Check for line completions within this card
                let mut max_line_score = 0;
                let mut best_line_numbers = Vec::new();
                
                // Check each row in the card (3 rows)
                for row_index in 0..3 {
                    let row = &assignment.card_data[row_index];
                    let mut row_extracted_numbers = Vec::new();
                    
                    for &number in row.iter().flatten() {
                        if board_numbers.contains(&number) {
                            row_extracted_numbers.push(number);
                        }
                    }
                    
                    // Update max line score if this row has more extracted numbers
                    if row_extracted_numbers.len() > max_line_score {
                        max_line_score = row_extracted_numbers.len();
                        best_line_numbers = row_extracted_numbers;
                    }
                }
                
                // Return the highest line score (2, 3, 4, or 5) and the numbers that made it
                if max_line_score >= 2 {
                    (max_line_score as Number, best_line_numbers)
                } else {
                    (0, Vec::new())
                }
            } else {
                (0, Vec::new())
            }
        };
        
        // Iterate through all card assignments
        for (card_id, assignment) in card_assignments {
            let (card_score, contributing_numbers) = calculate_card_score(assignment);
            
            // Update the global score to the highest score achieved by any card
            if card_score > global_score {
                global_score = card_score;
            }
            
            // Only include cards that have achieved a meaningful score (>= 2) and have contributing numbers
            if card_score >= 2 && !contributing_numbers.is_empty() {
                card_details.push((card_id.clone(), contributing_numbers));
            }
        }
        
        // Only return results if the global score is greater than the current published score
        if global_score > current_published_score {
            // Filter card_details to only include cards that achieved the global score
            let filtered_card_details: Vec<(String, Vec<Number>)> = card_details.into_iter()
                .filter(|(card_id, _)| {
                    // Use the helper function to recalculate the score for this card
                    if let Some(assignment) = card_assignments.get(card_id) {
                        let (card_score, _) = calculate_card_score(assignment);
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
        
        // Store the previous published score value BEFORE any updates
        let previous_published_score = self.published_score;
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
            if current_line_score > previous_published_score { 
                new_scorecard = current_line_score;
            } else {
                new_scorecard = previous_published_score;
            }
        }

        // Return numbers to mark based on scorecard achievement
        // Only return numbers if we just achieved a NEW score (higher than previous)
        let is_new_achievement = new_scorecard > previous_published_score;
        
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

    // Calculate and update the best score from both board and card scores
    pub fn calculate_and_update_best_score(
        &mut self, 
        board: &Board, 
        card_manager: &CardAssignmentManager,
        current_working_score: Number
    ) -> Number {
        // Get board numbers internally
        let board_numbers = board.get_numbers();
        
        // Get card assignments directly from the card manager
        let card_assignments = card_manager.get_all_assignments();
        
        // Calculate board score internally
        let (boardscore_value, board_numbers_contributing) = self.board_calculate_score(board_numbers);
        
        // Calculate scores for all cards
        let (allcardscore_value, card_details) = self.allcards_calculate_score(board_numbers, card_assignments);
        
        let bestscore = std::cmp::max(allcardscore_value, boardscore_value);
        
        // Always maintain score_map by preserving existing achievements and only adding new ones
        // This ensures achievements are never lost between iterations
        if bestscore >= 2 {
            // If this is a new achievement level (higher than what we had before)
            if bestscore > current_working_score {
                // Special handling for BINGO (15) - it's a unique achievement, not a line progression
                if bestscore == NUMBERSPERCARD {
                    // BINGO is achieved - only store the BINGO achievement itself
                    let bingo_achievements = if allcardscore_value == NUMBERSPERCARD {
                        // Card achieved BINGO
                        card_details.iter()
                            .filter(|(_, numbers)| numbers.len() as Number == NUMBERSPERCARD)
                            .map(|(card_id, numbers)| {
                                ScoreAchievement {
                                    client_id: card_manager.get_client_id_for_card(card_id),
                                    card_id: card_id.clone(),
                                    numbers: numbers.clone(),
                                }
                            }).collect()
                    } else if boardscore_value == NUMBERSPERCARD {
                        // Board achieved BINGO
                        vec![ScoreAchievement {
                            client_id: "0000000000000000".to_string(),
                            card_id: "0000000000000000".to_string(),
                            numbers: board_numbers_contributing.clone(),
                        }]
                    } else {
                        Vec::new()
                    };
                    
                    if !bingo_achievements.is_empty() {
                        self.score_map.insert(NUMBERSPERCARD, bingo_achievements);
                    }
                } else {
                    // Regular line achievements (2-5) - store achievements for all levels from 2 to bestscore
                    // This ensures we don't lose previous achievements when reaching a higher level
                    
                    for achievement_level in 2..=bestscore {
                        // Only add this level if it doesn't already exist in score_map
                        if !self.score_map.contains_key(&achievement_level) {
                            let level_achievements = if achievement_level == bestscore {
                                // For the current best score, use the actual current achievements
                                if allcardscore_value >= boardscore_value {
                                    // Add card achievements that achieved this score level
                                    let mut achievements: Vec<ScoreAchievement> = card_details.iter()
                                        .filter(|(_, numbers)| numbers.len() as Number == achievement_level)
                                        .map(|(card_id, numbers)| {
                                            ScoreAchievement {
                                                client_id: card_manager.get_client_id_for_card(card_id),
                                                card_id: card_id.clone(),
                                                numbers: numbers.clone(),
                                            }
                                        }).collect();
                                    
                                    // If board score also meets this level, include it
                                    if boardscore_value == achievement_level {
                                        achievements.push(ScoreAchievement {
                                            client_id: "0000000000000000".to_string(),
                                            card_id: "0000000000000000".to_string(),
                                            numbers: board_numbers_contributing.clone(),
                                        });
                                    }
                                    achievements
                                } else if boardscore_value == achievement_level {
                                    // Only board achievement
                                    vec![ScoreAchievement {
                                        client_id: "0000000000000000".to_string(),
                                        card_id: "0000000000000000".to_string(),
                                        numbers: board_numbers_contributing.clone(),
                                    }]
                                } else {
                                    Vec::new()
                                }
                            } else {
                                // For lower achievement levels, we need to reconstruct what achievements existed
                                // This is for cases where we jump from level 2 to level 4, we need to fill in level 3
                                let mut level_achievements = Vec::new();
                                
                                // Check if any card achieved exactly this level
                                for (card_id, numbers) in &card_details {
                                    if numbers.len() as Number >= achievement_level {
                                        // Take only the first 'achievement_level' numbers for this level
                                        let level_numbers: Vec<Number> = numbers.iter()
                                            .take(achievement_level as usize)
                                            .copied()
                                            .collect();
                                        level_achievements.push(ScoreAchievement {
                                            client_id: card_manager.get_client_id_for_card(card_id),
                                            card_id: card_id.clone(),
                                            numbers: level_numbers,
                                        });
                                    }
                                }
                                
                                // Check if board achieved this level
                                if boardscore_value >= achievement_level {
                                    // For board, take the first 'achievement_level' numbers from the contributing numbers
                                    let level_numbers: Vec<Number> = board_numbers_contributing.iter()
                                        .take(achievement_level as usize)
                                        .copied()
                                        .collect();
                                    level_achievements.push(ScoreAchievement {
                                        client_id: "0000000000000000".to_string(),
                                        card_id: "0000000000000000".to_string(),
                                        numbers: level_numbers,
                                    });
                                }
                                
                                level_achievements
                            };
                            
                            if !level_achievements.is_empty() {
                                self.score_map.insert(achievement_level, level_achievements);
                            }
                        }
                    }
                }
            }
            // Note: We do NOT touch existing achievements in score_map - they are preserved permanently
        }
        
        // Only update the published score when it actually increases
        if bestscore > current_working_score {
            self.update_scorecard(bestscore);
            bestscore // Return the new working score
        } else {
            current_working_score // Return the unchanged working score
        }
    }
}
