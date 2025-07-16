use crate::defs::{Number, FIRSTNUMBER, LASTNUMBER};
use serde::{Deserialize, Serialize};
use rand::{rng, Rng};

#[derive(Serialize, Deserialize, Clone)]
pub struct Pouch {
    pub numbers: Vec<Number>,
    pub remaining: usize,
}

impl Default for Pouch {
    fn default() -> Self {
        Self::new()
    }
}

impl Pouch {
    pub fn new() -> Self {
        let numbers: Vec<Number> = (FIRSTNUMBER..=LASTNUMBER).collect();
        let remaining = numbers.len();
        Pouch {
            numbers,
            remaining,
        }
    }
    
    pub fn len(&self) -> usize {
        self.remaining
    }
    
    pub fn is_empty(&self) -> bool {
        self.numbers.is_empty()
    }
    
    fn remove(&mut self, index: usize) -> Number {
        let number = self.numbers.remove(index);
        self.remaining = self.numbers.len();
        number
    }

    pub fn extract(&mut self) -> Number {
        if self.is_empty() {
            0 // Return 0 if pouch is empty
        } else {
            let random_index = rng().random_range(0..self.len());
            self.remove(random_index)
        }
    }
}
