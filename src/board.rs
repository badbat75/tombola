// tombola/src/board.rs
// This module handles the Board implementation and prize checking logic for the Tombola game.

use crate::defs::{BOARDCONFIG, NUMBERSPERCARD, Number};
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
struct NumberEntry {
    number: Number,
    is_marked: bool,
}

impl NumberEntry {
    pub fn number(&self) -> Number {
        self.number
    }
    
    pub fn is_marked(&self) -> bool {
        self.is_marked
    }
}

// This struct represents the board in the Tombola game.
pub struct Board(Vec<NumberEntry>);

// Implement general-purpose methods for the Board struct.
impl Board {
    pub fn new() -> Self {
        Board(Vec::new())
    }
    
    pub fn push(&mut self, entry: Number, scorecard: &mut Number) {
        self.0.push(NumberEntry {
            number: entry,
            is_marked: false,
        });
        
        // Automatically check for prizes when a number is added
        self.tombola_prize_check(entry, scorecard);
    }
    
    pub fn get_numbers(&self) -> Vec<Number> {
        self.0.iter().map(NumberEntry::number).collect()
    }
    
    pub fn len(&self) -> usize {
        self.0.len()
    }
    
    pub fn get_sorted_entries(&self) -> Vec<(Number, bool)> {
        let mut sorted: Vec<_> = self.0.iter().map(|entry| (entry.number(), entry.is_marked())).collect();
        sorted.sort_by_key(|&(number, _)| number);
        sorted
    }
    
    pub fn get_last_numbers(&self, n: usize) -> Vec<Number> {
        if self.0.len() <= 1 {
            return Vec::new();
        }

        let available_previous = self.0.len() - 1;
        let numbers_to_show = std::cmp::min(n, available_previous);
        let start_index = self.0.len() - numbers_to_show - 1;
        let end_index = self.0.len() - 1;

        let mut result: Vec<Number> = self.0[start_index..end_index]
            .iter()
            .map(NumberEntry::number)
            .collect();
        result.reverse();
        result
    }
    
    pub fn tombola_prize_check(&mut self, extracted: Number, scorecard: &mut Number) {
        let numbers_per_row = (BOARDCONFIG.cols_per_card * BOARDCONFIG.cards_per_row) as i8;
        
        // Store the previous scorecard value BEFORE any updates
        let previous_scorecard = *scorecard;
        
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
                    
                    if self.0.iter().any(|entry| entry.number == number as Number) {
                        card_numbers_found += 1;
                    }
                }
            }
            
            // If this card is complete (15 numbers), set scorecard to 15
            if card_numbers_found == NUMBERSPERCARD {
                *scorecard = NUMBERSPERCARD;
                bingo_found = true;
                break;
            }
        }
        
        // Only do line scoring if no BINGO was found
        if !bingo_found {
            // Count numbers in the same row and card as the extracted number (for line scoring)
            let mut same_row_card_count = 0;
            
            for entry in &self.0 {
                let num = entry.number;
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
            if current_line_score > *scorecard { 
                *scorecard = current_line_score;
            }
        }

        // Mark numbers based on scorecard achievement
        // Only mark if we just achieved a NEW score (higher than previous)
        let is_new_achievement = *scorecard > previous_scorecard;
        
        if is_new_achievement {
            match *scorecard {
                2..=5 => {
                    // First, unmark all numbers (reset previous markings)
                    for entry in &mut self.0 {
                        entry.is_marked = false;
                    }
                    
                    // Only mark numbers that are actually part of the current scoring line
                    for entry in &mut self.0 {
                        let num = entry.number;
                        
                        let num_ypos = (num as i8 - 1) / numbers_per_row + 1;
                        let num_xpos = (num as i8 - 1) % numbers_per_row;
                        let num_card = num_xpos / BOARDCONFIG.cols_per_card as i8;
                        
                        // Only mark if it's in the same row and card as the extracted number
                        if num_ypos == extracted_ypos && num_card == extracted_card {
                            entry.is_marked = true;
                        }
                    }
                },
                x if x == NUMBERSPERCARD => {
                    // First, unmark all numbers (BINGO overrides everything)
                    for entry in &mut self.0 {
                        entry.is_marked = false;
                    }
                    
                    // Mark all numbers in the completed card
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
                                
                                if self.0.iter().any(|entry| entry.number == number as Number) {
                                    card_numbers_found += 1;
                                }
                            }
                        }
                        
                        // If this card is complete, mark all its extracted numbers
                        if card_numbers_found == NUMBERSPERCARD {
                            for entry in &mut self.0 {
                                if card_numbers.contains(&entry.number) {
                                    entry.is_marked = true;
                                }
                            }
                            break;
                        }
                    }
                },
                _ => {}
            }
        }
    }
}
