// tombola/src/board.rs
// This module handles the Board implementation for the Tombola game.

use crate::defs::Number;
use crate::score::ScoreCard;
use std::collections::HashSet;

// This struct represents the board in the Tombola game.
pub struct Board {
    numbers: Vec<Number>,
    scorecard: ScoreCard,
    marked_numbers: HashSet<Number>,
}

// Implement general-purpose methods for the Board struct.
impl Board {
    pub fn new() -> Self {
        Board {
            numbers: Vec::new(),
            scorecard: ScoreCard::new(),
            marked_numbers: HashSet::new(),
        }
    }
    
    pub fn push(&mut self, entry: Number) {
        self.numbers.push(entry);
        let (new_score, numbers_to_mark) = self.scorecard.board_calculate_score(&self.numbers);
        
        // Update the scorecard score
        self.scorecard = ScoreCard::with_score(new_score);
        
        // Update marked numbers based on scoring
        if !numbers_to_mark.is_empty() {
            self.marked_numbers.clear();
            for number in numbers_to_mark {
                self.marked_numbers.insert(number);
            }
        }
    }
    
    pub fn get_numbers(&self) -> &Vec<Number> {
        &self.numbers
    }
    
    pub fn len(&self) -> usize {
        self.numbers.len()
    }
    
    pub fn get_scorecard(&self) -> Number {
        self.scorecard.get_scorecard()
    }
    
    pub fn get_sorted_entries(&self) -> Vec<(Number, bool)> {
        let mut sorted: Vec<_> = self.numbers.iter()
            .map(|&number| (number, self.marked_numbers.contains(&number)))
            .collect();
        sorted.sort_by_key(|&(number, _)| number);
        sorted
    }
    
    pub fn get_last_numbers(&self, n: usize) -> Vec<Number> {
        if self.numbers.len() <= 1 {
            return Vec::new();
        }

        let available_previous = self.numbers.len() - 1;
        let numbers_to_show = std::cmp::min(n, available_previous);
        let start_index = self.numbers.len() - numbers_to_show - 1;
        let end_index = self.numbers.len() - 1;

        let mut result: Vec<Number> = self.numbers[start_index..end_index].to_vec();
        result.reverse();
        result
    }
}
