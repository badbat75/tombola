// tombola/src/board.rs
// This module handles the Board implementation for the Tombola game.

use crate::defs::Number;
use crate::score::ScoreCard;
use std::collections::HashSet;
use serde::{Deserialize, Serialize};

// This struct represents the board in the Tombola game.
#[derive(Serialize, Deserialize, Clone)]
pub struct Board {
    numbers: Vec<Number>,
    marked_numbers: HashSet<Number>,
}

// Implement general-purpose methods for the Board struct.
impl Default for Board {
    fn default() -> Self {
        Self::new()
    }
}

impl Board {
    pub fn new() -> Self {
        Board {
            numbers: Vec::new(),
            marked_numbers: HashSet::new(),
        }
    }
    
    pub fn push(&mut self, entry: Number, scorecard: &ScoreCard) -> Number {
        self.numbers.push(entry);
        
        // Calculate score and numbers to mark
        let (score, numbers_to_mark) = scorecard.board_calculate_score(&self.numbers);
        
        // Update marked numbers based on scoring
        self.update_marked_numbers(numbers_to_mark);
        
        score
    }
    
    pub fn push_simple(&mut self, entry: Number) {
        self.numbers.push(entry);
    }
    
    pub fn update_marked_numbers(&mut self, numbers_to_mark: Vec<Number>) {
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
    
    pub fn is_empty(&self) -> bool {
        self.numbers.is_empty()
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

        // Get the last n numbers excluding the current (last) number
        let exclude_current = &self.numbers[..self.numbers.len() - 1];
        exclude_current
            .iter()
            .rev()
            .take(n)
            .copied()
            .collect()
    }
}
