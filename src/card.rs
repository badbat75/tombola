use crate::defs::{Number, FIRSTNUMBER, LASTNUMBER, CARDSNUMBER, BOARDCONFIG};

use std::collections::HashSet;
use std::collections::hash_map::DefaultHasher;
use std::hash::Hasher;
use rand::seq::SliceRandom;
use rand::rng;

#[derive(Debug, Clone)]
pub struct TombolaGenerator;

#[derive(Debug, Clone)]
pub struct CardWithId {
    pub id: u64,
    pub card: Card,
}

pub type Card = Vec<Vec<Option<Number>>>;  // BOARDCONFIG.rows_per_card rows × (LASTNUMBER/10) columns

impl TombolaGenerator {
    pub fn new() -> Self {
        Self
    }

    pub fn generate_card_group(&self) -> Vec<Card> {
        let columns = ((LASTNUMBER - FIRSTNUMBER + 1) / 10) as usize;  // Dynamic column calculation
        let numbers_per_card = ((LASTNUMBER - FIRSTNUMBER + 1) / CARDSNUMBER) as usize;
        
        // Step 1: Calculate number distribution per column per card
        let distribution = self.calculate_column_distribution(numbers_per_card, columns);
        
        // Step 2: Create allocation matrix with anti-adjacency
        let allocation_matrix = self.create_allocation_matrix(distribution);
        
        // Step 3: Distribute actual numbers
        let cards = self.distribute_numbers(allocation_matrix);
        
        // Step 4: Position numbers in cards respecting row constraints
        let mut cards = self.position_numbers_in_cards(cards);
        
        // Step 5: Randomize the order of the 6 cards
        let mut rng = rng();
        cards.shuffle(&mut rng);
        
        cards
    }

    fn calculate_column_distribution(&self, numbers_per_card: usize, columns: usize) -> (usize, usize) {
        // Calculate how many columns must have 2 numbers vs 1 number
        // Equation: a×2 + b×1 = numbers_per_card, where a+b = columns
        let columns_with_2_numbers = numbers_per_card - columns;
        let columns_with_1_number = columns - columns_with_2_numbers;
        (columns_with_2_numbers, columns_with_1_number)
    }

    fn create_allocation_matrix(&self, distribution: (usize, usize)) -> Vec<Vec<Number>> {
        let columns = ((LASTNUMBER - FIRSTNUMBER + 1) / 10) as usize;
        let (_columns_with_2_numbers, _columns_with_1_number) = distribution;
        let mut matrix = vec![vec![2 as Number; columns]; CARDSNUMBER as usize];

        // Anti-adjacency pattern for columns with 1 number
        let single_number_pattern = vec![
            vec![0, 3, 6],  // Card 1: columns 1,4,7
            vec![1, 4, 7],  // Card 2: columns 2,5,8  
            vec![2, 5, 8],  // Card 3: columns 3,6,9
            vec![0, 4, 8],  // Card 4: columns 1,5,9
            vec![1, 5, 6],  // Card 5: columns 2,6,7
            vec![2, 3, 7],  // Card 6: columns 3,4,8
        ];

        // Apply pattern for columns with 1 number
        for (card_idx, positions) in single_number_pattern.iter().enumerate() {
            for &pos in positions {
                if pos < columns {  // Verify position is valid
                    matrix[card_idx][pos] = 1;
                }
            }
        }

        // With uniform distribution strategy:
        // We'll temporarily borrow number 90 from column 8 and assign it to column 0
        // This makes all columns have exactly 10 numbers each for uniform allocation
        // Number 90 will be moved back to column 8 during the final positioning phase
        
        // Now all columns have 10 numbers each, so we can use uniform allocation
        // Each card gets 15 numbers total, distributed as 6 columns with 2 numbers + 3 columns with 1 number
        
        matrix
    }

    fn distribute_numbers(&self, allocation_matrix: Vec<Vec<Number>>) -> Vec<Vec<Vec<Number>>> {
        let columns = ((LASTNUMBER - FIRSTNUMBER + 1) / 10) as usize;
        let mut cards_numbers = vec![vec![Vec::new(); columns]; CARDSNUMBER as usize];
        let mut rng = rng();

        // Uniform distribution strategy:
        // - Column 0 normally has 9 numbers (1-9)
        // - Column 8 normally has 11 numbers (80-90)
        // - To achieve uniform allocation, we temporarily borrow number 90 from column 8
        //   and assign it to column 0, making both columns have 10 numbers each
        // - Number 90 will be moved back to column 8 in the positioning phase
        
        // For each column, distribute its numbers according to tombola rules
        for col in 0..columns {
            let mut column_numbers: Vec<Number> = if col == 0 {
                // First column: 1-9 + temporarily borrow 90 from column 8
                let mut numbers = (1..=9).collect::<Vec<Number>>();
                numbers.push(90);  // Temporarily add 90 to make uniform distribution
                numbers
            } else if col == 8 {
                // Last column (9th): 80-89 (excluding 90 which is temporarily in column 0)
                (80..=89).collect()
            } else {
                // Other columns: (col * 10) to (col * 10 + 9)
                let start = col * 10;
                let end = start + 9;
                (start as Number..=end as Number).collect()
            };
            
            column_numbers.shuffle(&mut rng);

            let mut number_index = 0;
            for card in 0..CARDSNUMBER as usize {
                let quantity = allocation_matrix[card][col] as usize;
                for _ in 0..quantity {
                    if number_index < column_numbers.len() {
                        cards_numbers[card][col].push(column_numbers[number_index]);
                        number_index += 1;
                    }
                }
                // Sort numbers in each column of each card
                cards_numbers[card][col].sort();
            }
        }

        cards_numbers
    }

    fn position_numbers_in_cards(&self, mut cards_numbers: Vec<Vec<Vec<Number>>>) -> Vec<Card> {
        let columns = ((LASTNUMBER - FIRSTNUMBER + 1) / 10) as usize;
        let mut rng = rng();
        
        // Move number 90 from column 0 back to column 8
        // This completes the uniform distribution strategy by putting 90 in its correct final position
        for card_idx in 0..CARDSNUMBER as usize {
            // Find and remove 90 from column 0
            if let Some(pos) = cards_numbers[card_idx][0].iter().position(|&x| x == 90) {
                cards_numbers[card_idx][0].remove(pos);
                // Add 90 to column 8 and keep it sorted
                cards_numbers[card_idx][8].push(90);
                cards_numbers[card_idx][8].sort();
            }
        }
        
        let mut cards = Vec::new();

        for card_numbers in cards_numbers {
            let mut card = vec![vec![None; columns]; BOARDCONFIG.rows_per_card as usize];
            
            // Create a strategy to ensure each row has exactly cols_per_card numbers
            let row_assignment = self.calculate_row_assignments(&card_numbers, columns);
            
            // Position numbers according to the calculated assignment
            for col in 0..columns {
                let column_numbers = &card_numbers[col];
                
                for (i, &number) in column_numbers.iter().enumerate() {
                    if i < row_assignment[col].len() {
                        let row = row_assignment[col][i];
                        card[row][col] = Some(number);
                    }
                }
            }

            // Randomize the rows in the card after positioning
            card.shuffle(&mut rng);

            cards.push(card);
        }

        cards
    }

    fn validate_cards(&self, cards: &[CardWithId]) -> bool {
        for card_with_id in cards {
            let card = &card_with_id.card;
            // Verify numbers per row
            for row in card {
                let numbers_in_row = row.iter().filter(|cell| cell.is_some()).count();
                if numbers_in_row != BOARDCONFIG.cols_per_card as usize {
                    return false;
                }
            }
            
            // Verify total numbers per card
            let total_numbers: usize = card.iter()
                .flat_map(|row| row.iter())
                .filter(|cell| cell.is_some())
                .count();
            
            if total_numbers != ((LASTNUMBER - FIRSTNUMBER + 1) / CARDSNUMBER) as usize {
                return false;
            }
        }
        true
    }

    fn print_cards(&self, cards: &[CardWithId]) {
        for (i, card_with_id) in cards.iter().enumerate() {
            println!("=== CARD {} (ID: {:016X}) ===", i + 1, card_with_id.id);
            for row in &card_with_id.card {
                for cell in row {
                    match cell {
                        Some(number) => print!("{:3} ", number),
                        None => print!("  . "),
                    }
                }
                
                // Check if row has exactly cols_per_card numbers
                let numbers_in_row = row.iter().filter(|cell| cell.is_some()).count();
                let is_valid = numbers_in_row == BOARDCONFIG.cols_per_card as usize;
                println!(" {}", if is_valid { "✓" } else { "✗" });
            }
            println!();
        }
    }

    fn calculate_row_assignments(&self, card_numbers: &[Vec<Number>], columns: usize) -> Vec<Vec<usize>> {
        let mut row_assignments = vec![Vec::new(); columns];
        let mut row_counts = vec![0; BOARDCONFIG.rows_per_card as usize];
        
        // Process each column and assign its numbers to rows
        for col in 0..columns {
            let numbers_in_column = card_numbers[col].len();
            
            // Assign each number in this column to the row with the fewest numbers
            for _ in 0..numbers_in_column {
                // Find the row with the minimum count
                let min_count = *row_counts.iter().min().unwrap();
                let target_row = row_counts.iter().position(|&count| count == min_count).unwrap();
                
                row_assignments[col].push(target_row);
                row_counts[target_row] += 1;
            }
        }
        
        row_assignments
    }

    pub fn generate_cards(&self, requested_cards: usize) -> Vec<CardWithId> {
        let mut all_cards = Vec::new();
        let mut remaining_cards = requested_cards;
        let mut rng = rng();
        let mut global_ids = HashSet::new();
        let mut total_regenerations = 0;

        // Generate complete blocks of CARDSNUMBER cards
        while remaining_cards > CARDSNUMBER as usize {
            let mut block = self.generate_card_group_with_ids();
            
            // Check for global duplicates across all generated blocks
            let mut has_global_duplicates = false;
            for card_with_id in &block {
                if global_ids.contains(&card_with_id.id) {
                    has_global_duplicates = true;
                    break;
                }
            }
            
            if has_global_duplicates {
                println!("Global duplicate ID detected across blocks, regenerating block");
                total_regenerations += 1;
                continue; // Regenerate this block
            }
            
            // Add IDs to global set
            for card_with_id in &block {
                global_ids.insert(card_with_id.id);
            }
            
            all_cards.append(&mut block);
            remaining_cards -= CARDSNUMBER as usize;
        }

        // Handle remaining cards
        if remaining_cards > 0 {
            loop {
                let mut final_block = self.generate_card_group_with_ids();
                
                // Check for global duplicates
                let mut has_global_duplicates = false;
                for card_with_id in &final_block {
                    if global_ids.contains(&card_with_id.id) {
                        has_global_duplicates = true;
                        break;
                    }
                }
                
                if has_global_duplicates {
                    println!("Global duplicate ID detected in final block, regenerating");
                    total_regenerations += 1;
                    continue; // Regenerate this block
                }
                
                // If we need fewer cards than a complete block, randomly select them
                if remaining_cards < CARDSNUMBER as usize {
                    final_block.shuffle(&mut rng);
                    final_block.truncate(remaining_cards);
                }
                
                all_cards.append(&mut final_block);
                break;
            }
        }

        if total_regenerations > 0 {
            println!("Total block regenerations due to global duplicates: {}", total_regenerations);
        }

        all_cards
    }

    fn generate_card_id(&self, card: &Card) -> u64 {
        let mut hasher = DefaultHasher::new();
        
        // Hash the card content in a deterministic way
        for row in card {
            for cell in row {
                match cell {
                    Some(number) => {
                        hasher.write_u8(*number);
                    }
                    None => {
                        hasher.write_u8(0); // Use 0 to represent None
                    }
                }
            }
        }
        
        hasher.finish()
    }

    pub fn generate_card_group_with_ids(&self) -> Vec<CardWithId> {
        const MAX_RETRIES: usize = 100;
        let mut attempt = 0;
        
        loop {
            attempt += 1;
            let cards = self.generate_card_group();
            let mut cards_with_ids = Vec::new();
            let mut seen_ids = HashSet::new();
            let mut has_duplicates = false;
            
            // Generate IDs for all cards and check for duplicates
            for card in cards {
                let id = self.generate_card_id(&card);
                if seen_ids.contains(&id) {
                    has_duplicates = true;
                    break;
                }
                seen_ids.insert(id);
                cards_with_ids.push(CardWithId { id, card });
            }
            
            if !has_duplicates {
                return cards_with_ids;
            }
            
            if attempt >= MAX_RETRIES {
                eprintln!("Warning: Could not generate unique card IDs after {} attempts", MAX_RETRIES);
                eprintln!("Proceeding with potentially duplicate IDs");
                return cards_with_ids;
            }
            
            println!("Duplicate card ID detected, regenerating group (attempt {})", attempt);
        }
    }
}