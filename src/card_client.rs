use tombola::defs::{NUMBERSPERCARD};
use tombola::score::ScoreCard;
use tombola::board::Board;
use tombola::pouch::Pouch;
use tombola::config::ClientConfig;

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
            Err(format!("Registration failed: {error_text}").into())
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
            let board: Board = response.json().await?;
            Ok(board.get_numbers().clone())
        } else {
            let error_response: ErrorResponse = response.json().await?;
            Err(format!("Failed to get board: {}", error_response.error).into())
        }
    }

    /// Get the current scorecard from the server
    pub async fn get_scorecard(&self) -> Result<ScoreCard, Box<dyn std::error::Error>> {
        self.ensure_registered()?;
        
        let url = format!("{}/scoremap", self.server_url);
        let response = self
            .http_client
            .get(&url)
            .header("X-Client-ID", self.client_id.as_ref().unwrap())
            .send()
            .await?;

        if response.status().is_success() {
            let scorecard: ScoreCard = response.json().await?;
            Ok(scorecard)
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
            let pouch: Pouch = response.json().await?;
            let remaining_count = pouch.len();
            Ok((pouch.numbers, remaining_count))
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

    /// Get running game ID and creation details
    pub async fn get_game_id(&self) -> Result<String, Box<dyn std::error::Error>> {
        let url = format!("{}/runninggameid", self.server_url);
        let response = self
            .http_client
            .get(&url)
            .send()
            .await?;

        if response.status().is_success() {
            let game_info: serde_json::Value = response.json().await?;
            if let (Some(game_id), Some(created_at)) = (
                game_info["game_id"].as_str(),
                game_info["created_at"].as_str()
            ) {
                Ok(format!("{}, started at: {}", game_id, created_at))
            } else {
                Err("Game ID or creation time not found in response".into())
            }
        } else {
            let error_response: ErrorResponse = response.json().await?;
            Err(format!("Failed to get game ID: {}", error_response.error).into())
        }
    }

    /// Start monitoring the game (polls server for updates)
    pub async fn start_monitoring(&self, interval_seconds: u64) -> Result<(), Box<dyn std::error::Error>> {
        self.ensure_registered()?;
        
        println!("🔄 Starting game monitoring (polling every {interval_seconds} seconds)...");
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
                            println!("⚠️  Failed to get board: {e}");
                        }
                    }
                }
                Err(e) => {
                    println!("❌ Failed to get status: {e}");
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
async fn main() {
    println!("🚀 Tombola Client Starting...");

    // Load client configuration
    let config = ClientConfig::load_or_default();
    let server_url = config.server_url();

    // Parse command line arguments
    let args: Vec<String> = std::env::args().collect();
    let client_name = args.get(1).unwrap_or(&config.client_name).clone();

    // Create and register client
    let mut client = TombolaClient::new(&client_name, &server_url);

    // Check for --nocard option
    if let Some(nocard_pos) = args.iter().position(|arg| arg == "--nocard") {
        if let Some(nocard_value) = args.get(nocard_pos + 1) {
            match nocard_value.parse::<u32>() {
                Ok(count) => {
                    client.set_nocard(count);
                    println!("🎴 Will request {count} cards during registration");
                }
                Err(_) => {
                    println!("❌ Invalid nocard value: '{nocard_value}'. Using default of 1 card.");
                    client.set_nocard(1);
                }
            }
        } else {
            println!("❌ --nocard flag requires a number. Using default of 1 card.");
            client.set_nocard(1);
        }
    }

    // Register with server
    let registration_result = client.register().await;
    match registration_result {
        Ok(()) => {
            println!("✅ Client registered successfully!");

            // Get assigned cards info once
            let assigned_cards = match client.list_assigned_cards().await {
                Ok(response) => response.cards,
                Err(e) => {
                    println!("❌ Failed to list assigned cards: {e}");
                    std::process::exit(1);
                }
            };

            if assigned_cards.is_empty() {
                println!("No cards assigned to this client.");
                return;
            }

            println!("\n� Starting live monitoring (updating every 2 seconds)...");
            println!("�📇 Your Cards ({} total)", assigned_cards.len());
            println!("💡 Numbers highlighted in \x1b[1;33myellow\x1b[0m have been extracted from the pouch");
            println!("🛑 Press Ctrl+C to stop monitoring\n");

            // Main monitoring loop
            loop {
                // Clear screen for better readability
                print!("\x1b[2J\x1b[1;1H");

                // Show timestamp
                println!("🕐 Last update: {}", chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC"));

                // Show Game ID
                let game_id_info = client.get_game_id().await.unwrap_or_else(|_| "Unknown".to_string());
                println!("🎮 Game ID: {}", game_id_info);

                // Get the current board (extracted numbers) from the server
                let extracted_numbers = match client.get_board().await {
                    Ok(board) => {
                        if !board.is_empty() {
                            println!("🎯 Extracted numbers ({}): {:?}", board.len(), board);
                        } else {
                            println!("🎯 No numbers extracted yet");
                        }
                        board
                    },
                    Err(e) => {
                        println!("⚠️  Warning: Failed to get board state: {e}");
                        Vec::new()
                    }
                };

                // Get the current scorecard from the server
                let scorecard = match client.get_scorecard().await {
                    Ok(scorecard) => {
                        if scorecard.published_score > 0 {
                            println!("📊 Current scorecard: {} (achievements shown only if card ID is published in score map)", scorecard.published_score);
                        } else {
                            println!("📊 No scorecard yet");
                        }
                        scorecard
                    },
                    Err(e) => {
                        println!("⚠️  Warning: Failed to get scorecard: {e}");
                        ScoreCard::new() // Default to empty ScoreCard if we can't get it
                    }
                };

                println!("\n📇 Your Cards ({} total):", assigned_cards.len());

                let mut bingo_cards = Vec::new();
                let mut card_achievements = Vec::new();

                for (index, card) in assigned_cards.iter().enumerate() {
                    match client.get_assigned_card(&card.card_id).await {
                        Ok(card_info) => {
                            let (is_bingo, achievements) = print_card_as_table_with_highlights(index + 1, &card_info.card_id, &card_info.card_data, &extracted_numbers, &scorecard);
                            if is_bingo {
                                bingo_cards.push(card_info.card_id.clone());
                            }
                            if !achievements.is_empty() {
                                card_achievements.push((card_info.card_id.clone(), achievements));
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
                        println!("   🎉 BINGO with Card ID: {card_id}");
                    }
                }

                if scorecard.published_score == NUMBERSPERCARD { return; } // Exit if BINGO achieved;

                // Wait for 2 seconds before next update
                std::thread::sleep(Duration::from_secs(2));
            }
        }
        Err(e) => {
            println!("❌ Registration failed: {e}");
            std::process::exit(1);
        }
    }
}

fn print_card_as_table_with_highlights(card_number: usize, card_id: &str, card_data: &[Vec<Option<u8>>], extracted_numbers: &[u8], scorecard: &ScoreCard) -> (bool, Vec<String>) {
    println!("\n┌────────────────────────────────────────────────────────────────────────────────┐");
    
    // Calculate proper spacing for the title to align the right border
    let title_text = format!("Card {card_number} - ID: {card_id}");
    let box_width = 78; // Total width of the box content area (counting the actual characters)
    let padding = if title_text.len() < box_width {
        box_width - title_text.len()
    } else {
        0
    };
    
    println!("│ {}{} │", title_text, " ".repeat(padding));
    println!("├────────┬────────┬────────┬────────┬────────┬────────┬────────┬────────┬────────┤");
    
    // Get the numbers that contributed to the highest published score for this card
    let highest_score_numbers = get_highest_score_numbers_for_card(scorecard, card_id);
    
    // Display card numbers, highlighting extracted ones and emphasizing highest score contributors
    let mut all_card_numbers = Vec::new();
    
    for row in card_data.iter() {
        print!("│");
        
        for cell in row {
            match cell {
                Some(number) => {
                    all_card_numbers.push(*number);
                    if extracted_numbers.contains(number) {
                        if highest_score_numbers.contains(number) {
                            // Yellow highlight for numbers that contributed to the highest published score
                            print!("{}   {:2}   {}|", tombola::defs::Colors::yellow(), number, tombola::defs::Colors::reset());
                        } else {
                            // Green for other extracted numbers
                            print!("{}   {:2}   {}|", tombola::defs::Colors::green(), number, tombola::defs::Colors::reset());
                        }
                    } else {
                        print!("   {number:2}   |");
                    }
                }
                None => print!("        │"),
            }
        }
        println!();
    }
    
    println!("└────────┴────────┴────────┴────────┴────────┴────────┴────────┴────────┴────────┘");
    
    // Get achievements from server scorecard instead of calculating locally
    let mut achievements_for_this_card = Vec::new();
    let mut is_bingo = false;
    
    // Check all scores in the scorecard for this card
    for (score, score_achievements) in scorecard.get_scoremap() {
        for achievement in score_achievements {
            if achievement.card_id == card_id {
                match score {
                    2 => achievements_for_this_card.push("2 in line".to_string()),
                    3 => achievements_for_this_card.push("3 in line".to_string()),
                    4 => achievements_for_this_card.push("4 in line".to_string()),
                    5 => achievements_for_this_card.push("5 in line".to_string()),
                    x if *x == NUMBERSPERCARD => {
                        achievements_for_this_card.push("BINGO".to_string());
                        is_bingo = true;
                    },
                    _ => {} // Handle any other scores
                }
            }
        }
    }
    
    // Display progress information
    if is_bingo {
        println!("\n🎉 \x1b[1;32mBINGO with Card ID {card_id} !!!!\x1b[0m 🎉");
    }
    
    // Display achievements from server scorecard
    if !achievements_for_this_card.is_empty() {
        println!("� Server-confirmed achievements:");
        for achievement in &achievements_for_this_card {
            println!("   ✅ {achievement}");
        }
    }
    
    (is_bingo, achievements_for_this_card)
}

// Helper function to get the numbers that contributed to the highest published score for a specific card
fn get_highest_score_numbers_for_card(scorecard: &ScoreCard, card_id: &str) -> Vec<u8> {
    let published_score = scorecard.published_score;
    
    // If no score has been published, return empty
    if published_score == 0 {
        return Vec::new();
    }
    
    // Look for this card in the score map for the published score
    if let Some(achievements) = scorecard.get_scoremap().get(&published_score) {
        for achievement in achievements {
            if achievement.card_id == card_id {
                return achievement.numbers.clone();
            }
        }
    }
    
    Vec::new()
}
