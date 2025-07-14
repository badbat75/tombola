use std::time::Duration;
use serde::{Deserialize, Serialize};
use tokio::time::sleep;

// Client registration request
#[derive(Debug, Serialize)]
struct RegisterRequest {
    name: String,
    client_type: String,
    nocard: Option<u32>,  // Number of cards to generate during registration
}

// Client registration response
#[derive(Debug, Deserialize)]
struct RegisterResponse {
    client_id: String,
    message: String,
}

// Generic API response structure
#[derive(Debug, Deserialize)]
struct BoardResponse {
    board: Vec<u8>,
}

#[derive(Debug, Deserialize)]
struct ScorecardResponse {
    scorecard: u8,
}

#[derive(Debug, Deserialize)]
struct PouchResponse {
    pouch: Vec<u8>,
    remaining: usize,
}

#[derive(Debug, Deserialize)]
struct ErrorResponse {
    error: String,
}

// Card generation request
#[derive(Debug, Serialize)]
struct GenerateCardsRequest {
    count: u32,
}

// Card generation response
#[derive(Debug, Deserialize)]
pub struct GenerateCardsResponse {
    pub cards: Vec<CardInfo>,
    pub message: String,
}

// Card info structure
#[derive(Debug, Deserialize)]
pub struct CardInfo {
    pub card_id: String,
    pub card_data: Vec<Vec<Option<u8>>>, // 2D vector of optional u8 values
}

// List assigned cards response
#[derive(Debug, Deserialize)]
pub struct ListAssignedCardsResponse {
    pub cards: Vec<AssignedCardInfo>,
}

// Assigned card info
#[derive(Debug, Deserialize)]
pub struct AssignedCardInfo {
    pub card_id: String,
    pub assigned_to: String,
}

// Tombola client structure
#[derive(Debug)]
pub struct TombolaClient {
    client_id: Option<String>,
    client_name: String,
    server_url: String,
    http_client: reqwest::Client,
    registered: bool,
    nocard: Option<u32>,  // Number of cards to generate during registration
}

impl TombolaClient {
    /// Create a new Tombola client
    pub fn new(name: &str, server_url: &str) -> Self {
        let http_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client_id: None,
            client_name: name.to_string(),
            server_url: server_url.to_string(),
            http_client,
            registered: false,
            nocard: None,
        }
    }

    /// Register the client with the server
    pub async fn register(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.registered {
            return Ok(());
        }

        let register_request = RegisterRequest {
            name: self.client_name.clone(),
            client_type: "tombola_client".to_string(),
            nocard: self.nocard, // Include nocard in the registration request
        };

        let url = format!("{}/register", self.server_url);
        println!("Registering client '{}' with server...", self.client_name);
        println!("🔍 Debug: Sending nocard = {:?}", self.nocard);

        let response = self
            .http_client
            .post(&url)
            .json(&register_request)
            .send()
            .await?;

        if response.status().is_success() {
            let register_response: RegisterResponse = response.json().await?;
            self.client_id = Some(register_response.client_id.clone());
            self.registered = true;
            
            println!("✅ Registration successful!");
            println!("   Client ID: {}", register_response.client_id);
            println!("   Message: {}", register_response.message);
            
            Ok(())
        } else {
            let error_text = response.text().await?;
            Err(format!("Registration failed: {}", error_text).into())
        }
    }

    /// Set the number of cards to generate during registration
    pub fn set_nocard(&mut self, count: u32) {
        self.nocard = Some(count);
    }

    /// Clear the nocard option
    pub fn clear_nocard(&mut self) {
        self.nocard = None;
    }

    /// Get the client ID (must be registered first)
    pub fn get_client_id(&self) -> Option<&String> {
        self.client_id.as_ref()
    }

    /// Check if client is registered
    pub fn is_registered(&self) -> bool {
        self.registered
    }

    /// Get the current board state from the server
    pub async fn get_board(&self) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        self.ensure_registered()?;
        
        let url = format!("{}/board", self.server_url);
        let response = self
            .http_client
            .get(&url)
            .header("X-Client-ID", self.client_id.as_ref().unwrap())
            .send()
            .await?;

        if response.status().is_success() {
            let board_response: BoardResponse = response.json().await?;
            Ok(board_response.board)
        } else {
            let error_response: ErrorResponse = response.json().await?;
            Err(format!("Failed to get board: {}", error_response.error).into())
        }
    }

    /// Get the current scorecard from the server
    pub async fn get_scorecard(&self) -> Result<u8, Box<dyn std::error::Error>> {
        self.ensure_registered()?;
        
        let url = format!("{}/scorecard", self.server_url);
        let response = self
            .http_client
            .get(&url)
            .header("X-Client-ID", self.client_id.as_ref().unwrap())
            .send()
            .await?;

        if response.status().is_success() {
            let scorecard_response: ScorecardResponse = response.json().await?;
            Ok(scorecard_response.scorecard)
        } else {
            let error_response: ErrorResponse = response.json().await?;
            Err(format!("Failed to get scorecard: {}", error_response.error).into())
        }
    }

    /// Get the current pouch state from the server
    pub async fn get_pouch(&self) -> Result<(Vec<u8>, usize), Box<dyn std::error::Error>> {
        self.ensure_registered()?;
        
        let url = format!("{}/pouch", self.server_url);
        let response = self
            .http_client
            .get(&url)
            .header("X-Client-ID", self.client_id.as_ref().unwrap())
            .send()
            .await?;

        if response.status().is_success() {
            let pouch_response: PouchResponse = response.json().await?;
            Ok((pouch_response.pouch, pouch_response.remaining))
        } else {
            let error_response: ErrorResponse = response.json().await?;
            Err(format!("Failed to get pouch: {}", error_response.error).into())
        }
    }

    /// Generate cards for the client
    pub async fn generate_cards(&self, count: u32) -> Result<GenerateCardsResponse, Box<dyn std::error::Error>> {
        self.ensure_registered()?;
        
        let request = GenerateCardsRequest { count };
        let url = format!("{}/generatecardsforme", self.server_url);
        
        let response = self
            .http_client
            .post(&url)
            .header("X-Client-ID", self.client_id.as_ref().unwrap())
            .json(&request)
            .send()
            .await?;

        if response.status().is_success() {
            let generate_response: GenerateCardsResponse = response.json().await?;
            Ok(generate_response)
        } else {
            let error_response: ErrorResponse = response.json().await?;
            Err(format!("Failed to generate cards: {}", error_response.error).into())
        }
    }

    /// List assigned cards for the client
    pub async fn list_assigned_cards(&self) -> Result<ListAssignedCardsResponse, Box<dyn std::error::Error>> {
        self.ensure_registered()?;
        
        let url = format!("{}/listassignedcards", self.server_url);
        let response = self
            .http_client
            .get(&url)
            .header("X-Client-ID", self.client_id.as_ref().unwrap())
            .send()
            .await?;

        if response.status().is_success() {
            let list_response: ListAssignedCardsResponse = response.json().await?;
            Ok(list_response)
        } else {
            let error_response: ErrorResponse = response.json().await?;
            Err(format!("Failed to list assigned cards: {}", error_response.error).into())
        }
    }

    /// Get a specific assigned card by ID
    pub async fn get_assigned_card(&self, card_id: &str) -> Result<CardInfo, Box<dyn std::error::Error>> {
        self.ensure_registered()?;
        
        let url = format!("{}/getassignedcard/{}", self.server_url, card_id);
        let response = self
            .http_client
            .get(&url)
            .header("X-Client-ID", self.client_id.as_ref().unwrap())
            .send()
            .await?;

        if response.status().is_success() {
            let card_info: CardInfo = response.json().await?;
            Ok(card_info)
        } else {
            let error_response: ErrorResponse = response.json().await?;
            Err(format!("Failed to get assigned card: {}", error_response.error).into())
        }
    }

    /// Get server status
    pub async fn get_status(&self) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
        self.ensure_registered()?;
        
        let url = format!("{}/status", self.server_url);
        let response = self
            .http_client
            .get(&url)
            .header("X-Client-ID", self.client_id.as_ref().unwrap())
            .send()
            .await?;

        if response.status().is_success() {
            let status: serde_json::Value = response.json().await?;
            Ok(status)
        } else {
            let error_response: ErrorResponse = response.json().await?;
            Err(format!("Failed to get status: {}", error_response.error).into())
        }
    }

    /// Start monitoring the game (polls server for updates)
    pub async fn start_monitoring(&self, interval_seconds: u64) -> Result<(), Box<dyn std::error::Error>> {
        self.ensure_registered()?;
        
        println!("🔄 Starting game monitoring (polling every {} seconds)...", interval_seconds);
        println!("   Client ID: {}", self.client_id.as_ref().unwrap());
        
        loop {
            match self.get_status().await {
                Ok(status) => {
                    println!("📊 Server Status: {}", serde_json::to_string_pretty(&status)?);
                    
                    // Also get the current board
                    match self.get_board().await {
                        Ok(board) => {
                            println!("🎯 Current Board ({} numbers): {:?}", board.len(), board);
                        }
                        Err(e) => {
                            println!("⚠️  Failed to get board: {}", e);
                        }
                    }
                }
                Err(e) => {
                    println!("❌ Failed to get status: {}", e);
                }
            }
            
            sleep(Duration::from_secs(interval_seconds)).await;
        }
    }

    /// Ensure the client is registered before making requests
    fn ensure_registered(&self) -> Result<(), Box<dyn std::error::Error>> {
        if !self.registered {
            Err("Client not registered. Call register() first.".into())
        } else {
            Ok(())
        }
    }
}

// Example usage and main function for testing
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Tombola Client Starting...");
    
    // Parse command line arguments
    let args: Vec<String> = std::env::args().collect();
    let client_name = args.get(1).unwrap_or(&"TestClient".to_string()).clone();
    let server_url = args.get(2).unwrap_or(&"http://127.0.0.1:3000".to_string()).clone();
    
    // Create and register client
    let mut client = TombolaClient::new(&client_name, &server_url);
    
    // Check for --nocard option
    if let Some(nocard_pos) = args.iter().position(|arg| arg == "--nocard") {
        if let Some(nocard_value) = args.get(nocard_pos + 1) {
            match nocard_value.parse::<u32>() {
                Ok(count) => {
                    client.set_nocard(count);
                    println!("🎴 Will request {} cards during registration", count);
                }
                Err(_) => {
                    println!("❌ Invalid nocard value: '{}'. Using default of 1 card.", nocard_value);
                    client.set_nocard(1);
                }
            }
        } else {
            println!("❌ --nocard flag requires a number. Using default of 1 card.");
            client.set_nocard(1);
        }
    }
    
    // Register with server
    match client.register().await {
        Ok(()) => {
            println!("✅ Client registered successfully!");
            
            // Get and display all assigned cards
            match client.list_assigned_cards().await {
                Ok(response) => {
                    if response.cards.is_empty() {
                        println!("No cards assigned to this client.");
                    } else {
                        println!("\n📇 Your Cards ({} total):", response.cards.len());
                        
                        // Get the current board (extracted numbers) from the server
                        let extracted_numbers = match client.get_board().await {
                            Ok(board) => {
                                if !board.is_empty() {
                                    println!("🎯 Extracted numbers: {:?}", board);
                                    println!("💡 Numbers highlighted in \x1b[1;33myellow\x1b[0m have been extracted from the pouch");
                                }
                                board
                            },
                            Err(e) => {
                                println!("⚠️  Warning: Failed to get board state: {}", e);
                                Vec::new()
                            }
                        };
                        
                        // Get the current scorecard from the server
                        let scorecard = match client.get_scorecard().await {
                            Ok(scorecard) => {
                                println!("📊 Current scorecard: {} (achievements shown only if scorecard < achievement level)", scorecard);
                                scorecard
                            },
                            Err(e) => {
                                println!("⚠️  Warning: Failed to get scorecard: {}", e);
                                0 // Default to 0 if we can't get scorecard
                            }
                        };
                        
                        let mut bingo_cards = Vec::new();
                        let mut line_achievements = Vec::new();
                        
                        for (index, card) in response.cards.iter().enumerate() {
                            match client.get_assigned_card(&card.card_id).await {
                                Ok(card_info) => {
                                    let (is_bingo, card_lines) = print_card_as_table_with_highlights(index + 1, &card_info.card_id, &card_info.card_data, &extracted_numbers, scorecard);
                                    if is_bingo {
                                        bingo_cards.push(card_info.card_id.clone());
                                    }
                                    if !card_lines.is_empty() {
                                        line_achievements.push((card_info.card_id.clone(), card_lines));
                                    }
                                }
                                Err(e) => {
                                    println!("   ❌ Failed to get card {} details: {}", index + 1, e);
                                }
                            }
                        }
                        
                        // Show BINGO summary if any cards have BINGO
                        if !bingo_cards.is_empty() {
                            println!("\n🏆 \x1b[1;32mCONGRATULATIONS! You have {} BINGO card(s)!\x1b[0m 🏆", bingo_cards.len());
                            for card_id in bingo_cards {
                                println!("   🎉 BINGO with Card ID: {}", card_id);
                            }
                        }
                        
                        // Show line achievements summary
                        if !line_achievements.is_empty() {
                            println!("\n📋 Line Achievements Summary:");
                            let mut total_lines = 0;
                            let mut five_in_line = 0;
                            let mut four_in_line = 0;
                            let mut three_in_line = 0;
                            let mut two_in_line = 0;
                            
                            for (card_id, lines) in &line_achievements {
                                println!("   Card {}: {} achievement(s)", card_id, lines.len());
                                for line in lines {
                                    println!("      • {}", line);
                                    total_lines += 1;
                                    if line.contains("5 in line") {
                                        five_in_line += 1;
                                    } else if line.contains("4 in line") {
                                        four_in_line += 1;
                                    } else if line.contains("3 in line") {
                                        three_in_line += 1;
                                    } else if line.contains("2 in line") {
                                        two_in_line += 1;
                                    }
                                }
                            }
                            
                            println!("\n📊 Total Line Statistics:");
                            if five_in_line > 0 {
                                println!("   🎯 5 in line: {}", five_in_line);
                            }
                            if four_in_line > 0 {
                                println!("   🎪 4 in line: {}", four_in_line);
                            }
                            if three_in_line > 0 {
                                println!("   🎭 3 in line: {}", three_in_line);
                            }
                            if two_in_line > 0 {
                                println!("   🎨 2 in line: {}", two_in_line);
                            }
                            println!("   📈 Total achievements: {}", total_lines);
                        }
                    }
                }
                Err(e) => println!("❌ Failed to list assigned cards: {}", e),
            }
        }
        Err(e) => {
            println!("❌ Registration failed: {}", e);
            std::process::exit(1);
        }
    }
    
    Ok(())
}

fn print_card_as_table_with_highlights(card_number: usize, card_id: &str, card_data: &Vec<Vec<Option<u8>>>, extracted_numbers: &Vec<u8>, scorecard: u8) -> (bool, Vec<String>) {
    println!("\n┌────────────────────────────────────────────────────────────────────────────────┐");
    
    // Calculate proper spacing for the title to align the right border
    let title_text = format!("Card {} - ID: {}", card_number, card_id);
    let box_width = 78; // Total width of the box content area (counting the actual characters)
    let padding = if title_text.len() < box_width {
        box_width - title_text.len()
    } else {
        0
    };
    
    println!("│ {}{} │", title_text, " ".repeat(padding));
    println!("├────────┬────────┬────────┬────────┬────────┬────────┬────────┬────────┬────────┤");
    
    // Track all numbers in the card and how many have been extracted
    let mut all_card_numbers = Vec::new();
    let mut extracted_count = 0;
    let mut lines_completed = Vec::new(); // Track completed lines (2, 3, 4, 5)
    
    for (row_index, row) in card_data.iter().enumerate() {
        print!("│");
        let mut row_extracted_count = 0;
        let mut row_total_count = 0;
        
        for cell in row {
            match cell {
                Some(number) => {
                    all_card_numbers.push(*number);
                    row_total_count += 1;
                    if extracted_numbers.contains(number) {
                        // Bold yellow text using ANSI escape codes
                        print!("\x1b[1;33m   {:2}   \x1b[0m│", number);
                        extracted_count += 1;
                        row_extracted_count += 1;
                    } else {
                        print!("   {:2}   │", number);
                    }
                }
                None => print!("        │"),
            }
        }
        println!();
        
        // Check for lines in this row (2, 3, 4, 5 numbers) - only show if scorecard < achievement level
        if row_total_count > 0 && row_extracted_count >= 2 {
            if row_extracted_count >= 5 && scorecard < 5 {
                lines_completed.push(format!("Row {} - 5 in line", row_index + 1));
            } else if row_extracted_count >= 4 && scorecard < 4 {
                lines_completed.push(format!("Row {} - 4 in line", row_index + 1));
            } else if row_extracted_count >= 3 && scorecard < 3 {
                lines_completed.push(format!("Row {} - 3 in line", row_index + 1));
            } else if row_extracted_count >= 2 && scorecard < 2 {
                lines_completed.push(format!("Row {} - 2 in line", row_index + 1));
            }
        }
    }
    
    println!("└────────┴────────┴────────┴────────┴────────┴────────┴────────┴────────┴────────┘");
    
    // Check for BINGO (all numbers in the card have been extracted)
    let is_bingo = !all_card_numbers.is_empty() && extracted_count == all_card_numbers.len();
    if is_bingo {
        println!("\n🎉 \x1b[1;32mBINGO with Card ID {} !!!!\x1b[0m 🎉", card_id);
    } else if !all_card_numbers.is_empty() {
        println!("📊 Progress: {}/{} numbers extracted ({:.1}%)", 
                 extracted_count, 
                 all_card_numbers.len(), 
                 (extracted_count as f64 / all_card_numbers.len() as f64) * 100.0);
    }
    
    // Display line achievements
    if !lines_completed.is_empty() {
        println!("🏃 Line achievements:");
        for line in &lines_completed {
            println!("   ✅ {}", line);
        }
    }
    
    (is_bingo, lines_completed)
}
