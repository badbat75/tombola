use crate::defs::{Number, FIRSTNUMBER, LASTNUMBER};
use serde::{Deserialize, Serialize};
use rand::{rng, Rng};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Pouch {
    pub numbers: Vec<Number>,
}

impl Default for Pouch {
    fn default() -> Self {
        Self::new()
    }
}

impl Pouch {
    #[must_use] pub fn new() -> Self {
        let numbers: Vec<Number> = (FIRSTNUMBER..=LASTNUMBER).collect();
        Pouch {
            numbers,
        }
    }

    #[must_use] pub fn len(&self) -> usize {
        self.numbers.len()
    }

    #[must_use] pub fn is_empty(&self) -> bool {
        self.numbers.is_empty()
    }

    fn remove(&mut self, index: usize) -> Number {
        self.numbers.remove(index)
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
