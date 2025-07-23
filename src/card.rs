use crate::defs::{Number, FIRSTNUMBER, LASTNUMBER, CARDSNUMBER, BOARDCONFIG};
use crate::client::ClientRegistry;
use crate::board::{BOARD_ID, board_client_id};

use std::collections::HashSet;
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::Hasher;
use rand::seq::SliceRandom;
use rand::rng;
use serde::{Deserialize, Serialize};

// Card generation request
#[derive(Debug, Deserialize)]
pub struct GenerateCardsRequest {
    pub count: u32,
}

// Card generation response
#[derive(Debug, Serialize)]
pub struct GenerateCardsResponse {
    pub cards: Vec<CardInfo>,
    pub message: String,
}

// Card info for responses
#[derive(Debug, Serialize)]
pub struct CardInfo {
    pub card_id: String,
    pub card_data: Vec<Vec<Option<u8>>>, // Changed to Option<u8> to match Card structure
}

// List assigned cards response
#[derive(Debug, Serialize)]
pub struct ListAssignedCardsResponse {
    pub cards: Vec<AssignedCardInfo>,
}

// Assigned card info
#[derive(Debug, Serialize)]
pub struct AssignedCardInfo {
    pub card_id: String,
    pub assigned_to: String,
}

// Card assignment storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CardAssignment {
    pub card_id: String,
    pub client_id: String,
    pub card_data: Card,
}

#[derive(Debug, Clone)]
pub struct CardManagement;

#[derive(Debug, Clone)]
pub struct CardWithId {
    pub id: u64,
    pub card: Card,
}

pub type Card = Vec<Vec<Option<Number>>>;  // BOARDCONFIG.rows_per_card rows × (LASTNUMBER/10) columns

impl Default for CardManagement {
    fn default() -> Self {
        Self::new()
    }
}

impl CardManagement {
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
        let cards = self.distribute_numbers(&allocation_matrix);

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
        let single_number_pattern = [
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

    fn distribute_numbers(&self, allocation_matrix: &[Vec<Number>]) -> Vec<Vec<Vec<Number>>> {
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
                // Other columns: Calculate range dynamically
                let start = col * 10;
                let end = start + 9;
                (start as Number..=end as Number).collect()
            };

            column_numbers.shuffle(&mut rng);

            let mut number_index = 0;
            for card in 0..CARDSNUMBER as usize {
                let quantity = allocation_matrix[card][col] as usize;
                if number_index + quantity <= column_numbers.len() {
                    // Use slice operations for better performance
                    let numbers_to_add = &column_numbers[number_index..number_index + quantity];
                    cards_numbers[card][col].extend_from_slice(numbers_to_add);
                    number_index += quantity;
                } else {
                    // Fallback to individual pushes if we don't have enough numbers
                    for _ in 0..quantity {
                        if number_index < column_numbers.len() {
                            cards_numbers[card][col].push(column_numbers[number_index]);
                            number_index += 1;
                        }
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
        for card_numbers in cards_numbers.iter_mut().take(CARDSNUMBER as usize) {
            // Find and remove 90 from column 0
            if let Some(pos) = card_numbers[0].iter().position(|&x| x == 90) {
                card_numbers[0].remove(pos);
                // Add 90 to column 8 and keep it sorted
                card_numbers[8].push(90);
                card_numbers[8].sort();
            }
        }

        let mut cards = Vec::new();

        for card_numbers in &cards_numbers {
            let mut card = vec![vec![None; columns]; BOARDCONFIG.rows_per_card as usize];

            // Create a strategy to ensure each row has exactly cols_per_card numbers
            let row_assignment = self.calculate_row_assignments(card_numbers, columns);

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
            let block = self.generate_card_group_with_ids();

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

            // Add IDs to global set and extend all_cards
            for card_with_id in &block {
                global_ids.insert(card_with_id.id);
            }

            all_cards.extend(block);
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

                all_cards.extend(final_block);
                break;
            }
        }

        if total_regenerations > 0 {
            println!("Total block regenerations due to global duplicates: {total_regenerations}");
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
            for card in &cards {
                let id = self.generate_card_id(card);
                if seen_ids.contains(&id) {
                    has_duplicates = true;
                    break;
                }
                seen_ids.insert(id);
            }

            if !has_duplicates {
                // Only create CardWithId structs if no duplicates found
                for card in cards {
                    let id = self.generate_card_id(&card);
                    cards_with_ids.push(CardWithId { id, card });
                }
                return cards_with_ids;
            }

            if attempt >= MAX_RETRIES {
                eprintln!("Warning: Could not generate unique card IDs after {MAX_RETRIES} attempts");
                eprintln!("Proceeding with potentially duplicate IDs");
                // Create CardWithId structs even with duplicates
                for card in cards {
                    let id = self.generate_card_id(&card);
                    cards_with_ids.push(CardWithId { id, card });
                }
                return cards_with_ids;
            }

            println!("Duplicate card ID detected, regenerating group (attempt {attempt})");
        }
    }

    /// Generate cards for a client and return card info structures
    pub fn generate_cards_for_client(&self, count: u32) -> Vec<CardInfo> {
        let cards_with_ids = self.generate_cards(count as usize);

        cards_with_ids.into_iter().map(|card_with_id| {
            let card_id_str = format!("{:016X}", card_with_id.id);
            CardInfo {
                card_id: card_id_str,
                card_data: card_with_id.card.iter().map(|row| {
                    row.to_vec()
                }).collect(),
            }
        }).collect()
    }

    /// Create a card assignment from a CardWithId and client_id
    pub fn create_card_assignment(&self, card_with_id: CardWithId, client_id: String) -> CardAssignment {
        let card_id_str = format!("{:016X}", card_with_id.id);
        CardAssignment {
            card_id: card_id_str,
            client_id,
            card_data: card_with_id.card,
        }
    }

    /// Convert Card to CardInfo for responses
    pub fn card_to_info(&self, card: &Card, card_id: String) -> CardInfo {
        CardInfo {
            card_id,
            card_data: card.iter().map(|row| {
                row.to_vec()
            }).collect(),
        }
    }

    /// Generate cards and assignments for a client
    pub fn generate_cards_and_assignments(&self, count: u32, client_id: String) -> (Vec<CardInfo>, Vec<CardAssignment>) {
        let cards_with_ids = self.generate_cards(count as usize);
        let mut card_infos = Vec::new();
        let mut assignments = Vec::new();

        for card_with_id in cards_with_ids {
            let card_id_str = format!("{:016X}", card_with_id.id);

            // Create card info for response first (takes ownership of card_id_str)
            let card_info = CardInfo {
                card_id: card_id_str.clone(),
                card_data: card_with_id.card.iter().map(|row| {
                    row.to_vec()
                }).collect(),
            };
            card_infos.push(card_info);

            // Create assignment (reuse card_id_str, take ownership of card data)
            let assignment = CardAssignment {
                card_id: card_id_str,
                client_id: client_id.clone(),
                card_data: card_with_id.card,
            };
            assignments.push(assignment);
        }

        (card_infos, assignments)
    }

    /// Generate cards and handle complete assignment process
    pub fn generate_and_assign_cards(&self, count: u32, client_id: &str, client_type: Option<&str>) -> (Vec<CardInfo>, Vec<String>, Vec<CardAssignment>) {
        // Check if this is a board client
        let is_board_client = client_type == Some("board");
        
        let cards_with_ids = if is_board_client {
            // For board clients, generate a special board card with BOARD_ID
            self.generate_board_card()
        } else {
            self.generate_cards(count as usize)
        };
        
        let mut card_infos = Vec::new();
        let mut client_card_ids = Vec::new();
        let mut assignments = Vec::new();

        for card_with_id in cards_with_ids {
            let card_id_str = if is_board_client {
                // Use the constant BOARD_ID for board clients
                BOARD_ID.to_string()
            } else {
                format!("{:016X}", card_with_id.id)
            };

            // Add to client's card list (clone needed for multiple uses)
            client_card_ids.push(card_id_str.clone());

            // Convert Card to CardInfo for response
            let card_info = CardInfo {
                card_id: card_id_str.clone(),
                card_data: card_with_id.card.iter().map(|row| {
                    row.to_vec()
                }).collect(),
            };
            card_infos.push(card_info);

            // Create assignment (takes ownership of remaining values)
            let assignment = CardAssignment {
                card_id: card_id_str,
                client_id: client_id.to_string(),
                card_data: card_with_id.card,
            };
            assignments.push(assignment);
        }

        (card_infos, client_card_ids, assignments)
    }

    // Generate a special board card for board clients
    fn generate_board_card(&self) -> Vec<CardWithId> {
        // Create a special card that represents the entire board
        let card = self.create_board_card();
        vec![CardWithId {
            id: 0, // Special ID that will be replaced with BOARD_ID
            card,
        }]
    }

    // Create the board card data representing the entire game board
    fn create_board_card(&self) -> Card {
        let mut card = Vec::new();
        let total_rows = BOARDCONFIG.cards_per_col as usize * BOARDCONFIG.rows_per_card as usize;
        let total_cols = BOARDCONFIG.cards_per_row as usize * BOARDCONFIG.cols_per_card as usize;

        for row in 0..total_rows {
            let mut card_row = Vec::new();
            for col in 0..total_cols {
                // Calculate the number for this position
                let number = FIRSTNUMBER + (row * total_cols + col) as Number;
                if number <= LASTNUMBER {
                    card_row.push(Some(number));
                } else {
                    card_row.push(None);
                }
            }
            card.push(card_row);
        }

        card
    }
}

// Card assignment manager - handles all card assignment logic
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CardAssignmentManager {
    assignments: HashMap<String, CardAssignment>,
    client_cards: HashMap<String, Vec<String>>,
}

impl Default for CardAssignmentManager {
    fn default() -> Self {
        Self::new()
    }
}

impl CardAssignmentManager {
    pub fn new() -> Self {
        Self {
            assignments: HashMap::new(),
            client_cards: HashMap::new(),
        }
    }

    pub fn assign_cards(&mut self, client_id: &str, count: u32) -> (Vec<CardInfo>, Vec<String>) {
        self.assign_cards_with_type(client_id, count, None)
    }

    pub fn assign_cards_with_type(&mut self, client_id: &str, count: u32, client_type: Option<&str>) -> (Vec<CardInfo>, Vec<String>) {
        let card_management = CardManagement::new();
        let (card_infos, client_card_ids, assignments) = card_management.generate_and_assign_cards(count, client_id, client_type);

        // Store assignments
        for assignment in assignments {
            let card_id = assignment.card_id.clone();
            self.assignments.insert(card_id, assignment);
        }

        // Store client's card IDs (clone needed since we return it too)
        self.client_cards.insert(client_id.to_string(), client_card_ids.clone());

        (card_infos, client_card_ids)
    }

    pub fn get_client_cards(&self, client_id: &str) -> Option<&Vec<String>> {
        self.client_cards.get(client_id)
    }

    pub fn get_card_assignment(&self, card_id: &str) -> Option<&CardAssignment> {
        self.assignments.get(card_id)
    }

    pub fn get_all_assignments(&self) -> &HashMap<String, CardAssignment> {
        &self.assignments
    }

    pub fn client_owns_card(&self, client_id: &str, card_id: &str) -> bool {
        if let Some(assignment) = self.assignments.get(card_id) {
            assignment.client_id == client_id
        } else {
            false
        }
    }

    pub fn get_client_assigned_cards(&self, client_id: &str) -> Vec<AssignedCardInfo> {
        self.client_cards.get(client_id)
            .map(|card_ids| {
                card_ids.iter().map(|card_id| {
                    AssignedCardInfo {
                        card_id: card_id.to_string(),
                        assigned_to: client_id.to_string(),
                    }
                }).collect()
            })
            .unwrap_or_default()
    }

    // Helper function to get client name from card ID
    pub fn get_client_name_for_card(&self, card_id: &str, client_registry: &ClientRegistry) -> String {
        if card_id == BOARD_ID {
            return "Board".to_string();
        }

        if let Some(assignment) = self.get_card_assignment(card_id) {
            if let Ok(clients) = client_registry.get_all_clients() {
                for client_info in clients {
                    if client_info.id == assignment.client_id {
                        return client_info.name.to_string();
                    }
                }
            }
        }
        "Unknown".to_string()
    }

    // Helper function to get client ID from card ID
    pub fn get_client_id_for_card(&self, card_id: &str) -> String {
        if let Some(assignment) = self.get_card_assignment(card_id) {
            return assignment.client_id.to_string();
        }
        
        // Fallback for unknown cards
        if card_id == BOARD_ID {
            return board_client_id(); // Backward compatibility fallback
        }
        
        "Unknown".to_string()
    }

}
