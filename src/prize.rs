// tombola/src/prize.rs
// This module handles the prize checking logic for the Tombola game.

use crate::defs::{NumberEntry, BOARDCONFIG, NUMBERSPERCARD};

pub fn tombola_prize_check(board: &mut [NumberEntry], extracted: u8, scorecard: &mut u8) {
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
        let mut card_numbers: Vec<u8> = Vec::new();
        
        // Check all numbers in this card
        for row in 0..BOARDCONFIG.rows_per_card as i8 {
            for col in 0..BOARDCONFIG.cols_per_card as i8 {
                let number = 1 + (card_row * BOARDCONFIG.rows_per_card as i8 + row) * numbers_per_row + 
                           card_col * BOARDCONFIG.cols_per_card as i8 + col;
                
                card_numbers.push(number as u8);
                
                if board.iter().any(|entry| entry.number == number as u8) {
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
        
        for entry in board.iter() {
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
                for entry in board.iter_mut() {
                    entry.is_marked = false;
                }
                
                // Only mark numbers that are actually part of the current scoring line
                for entry in board.iter_mut() {
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
                for entry in board.iter_mut() {
                    entry.is_marked = false;
                }
                
                // Mark all numbers in the completed card
                for card_index in 0..total_cards {
                    let card_row = card_index / BOARDCONFIG.cards_per_row as i8;
                    let card_col = card_index % BOARDCONFIG.cards_per_row as i8;
                    
                    let mut card_numbers_found = 0;
                    let mut card_numbers: Vec<u8> = Vec::new();
                    
                    // Collect all numbers in this card (regardless of extraction)
                    for row in 0..BOARDCONFIG.rows_per_card as i8 {
                        for col in 0..BOARDCONFIG.cols_per_card as i8 {
                            let number = 1 + (card_row * BOARDCONFIG.rows_per_card as i8 + row) * numbers_per_row + 
                                       card_col * BOARDCONFIG.cols_per_card as i8 + col;
                            
                            card_numbers.push(number as u8);
                            
                            if board.iter().any(|entry| entry.number == number as u8) {
                                card_numbers_found += 1;
                            }
                        }
                    }
                    
                    // Debug: Show what we found for this card
                    
                    // If this card is complete, mark all its extracted numbers
                    if card_numbers_found == NUMBERSPERCARD {
                        for entry in board.iter_mut() {
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