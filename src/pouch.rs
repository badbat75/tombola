use crate::defs::{Number, FIRSTNUMBER, LASTNUMBER};

pub struct Pouch {
    pub numbers: Vec<Number>,
}

impl Pouch {
    pub fn new() -> Self {
        Pouch {
            numbers: (FIRSTNUMBER..=LASTNUMBER).collect(),
        }
    }
    
    pub fn len(&self) -> usize {
        self.numbers.len()
    }
    
    pub fn is_empty(&self) -> bool {
        self.numbers.is_empty()
    }
    
    fn remove(&mut self, index: usize) -> Number {
        self.numbers.remove(index)
    }

    pub fn extract(&mut self) -> Number {
        if self.is_empty() {
            0 // Return 0 if pouch is empty
        } else {
            let random_index = rand::random_range(0..self.len());
            self.remove(random_index)
        }
    }
}
