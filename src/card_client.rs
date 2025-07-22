use tombola::defs::{NUMBERSPERCARD};
use tombola::score::ScoreCard;
use tombola::board::Board;
use tombola::pouch::Pouch;
use tombola::config::ClientConfig;

use std::time::Duration;
use serde::{Deserialize, Serialize};
use tokio::time::sleep;
use clap::Parser;

#[derive(Parser)]
#[command(name = env!("CARGO_BIN_NAME"))]
#[command(about = "Tombola Player Client - Monitor your cards and achievements")]
#[command(version = env!("CARGO_PKG_VERSION"))]
struct Args {
    /// Client name (default from config)
    #[arg(short, long)]
    name: Option<String>,

    /// Number of cards to request during registration
    #[arg(long)]
    nocard: Option<u32>,

    /// Exit after displaying the current state (no interactive loop)
    #[arg(long)]
    exit: bool,

    /// Game ID to connect to (required unless using --listgames)
    #[arg(long)]
    gameid: Option<String>,

    /// List active games and exit
    #[arg(long)]
    listgames: bool,
}

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
    game_id: Option<String>,  // Game ID to connect to
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
            game_id: None,
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

        let game_id = self.game_id.as_ref().ok_or("Game ID not set")?;
        let url = format!("{}/{}/register", self.server_url, game_id);
        println!("Registering client '{}' with server for game '{}'...", self.client_name, game_id);
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

    /// Set the game ID to connect to
    pub fn set_game_id(&mut self, game_id: String) {
        self.game_id = Some(game_id);
    }

    /// Get the current game ID
    pub fn current_game_id(&self) -> Option<&String> {
        self.game_id.as_ref()
    }

    /// Ensure a game ID is set
    fn ensure_game_id(&self) -> Result<&String, Box<dyn std::error::Error>> {
        self.game_id.as_ref().ok_or("Game ID not set. Use set_game_id() first.".into())
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
        let game_id = self.ensure_game_id()?;

        let url = format!("{}/{}/board", self.server_url, game_id);
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
        let game_id = self.ensure_game_id()?;

        let url = format!("{}/{}/scoremap", self.server_url, game_id);
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
        let game_id = self.ensure_game_id()?;

        let url = format!("{}/{}/pouch", self.server_url, game_id);
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
        let game_id = self.ensure_game_id()?;

        let request = GenerateCardsRequest { count };
        let url = format!("{}/{}/generatecardsforme", self.server_url, game_id);

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
        let game_id = self.ensure_game_id()?;

        let url = format!("{}/{}/listassignedcards", self.server_url, game_id);
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
        let game_id = self.ensure_game_id()?;

        let url = format!("{}/{}/getassignedcard/{}", self.server_url, game_id, card_id);
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
        let game_id = self.ensure_game_id()?;

        let url = format!("{}/{}/status", self.server_url, game_id);
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
        let url = format!("{}/gameslist", self.server_url);
        let response = self
            .http_client
            .get(&url)
            .send()
            .await?;

        if response.status().is_success() {
            let games_info: serde_json::Value = response.json().await?;

            if let Some(games) = games_info["games"].as_array() {
                // Find the first non-closed game
                for game in games {
                    if let (Some(game_id), Some(status), Some(start_date)) = (
                        game["game_id"].as_str(),
                        game["status"].as_str(),
                        game["start_date"].as_str()
                    ) {
                        if status != "Closed" {
                            return Ok(format!("{game_id}, started at: {start_date}"));
                        }
                    }
                }
                Err("No available games found".into())
            } else {
                Err("Invalid response format from games list endpoint".into())
            }
        } else {
            let error_response: ErrorResponse = response.json().await?;
            Err(format!("Failed to get game ID: {}", error_response.error).into())
        }
    }

    /// List active games
    pub async fn list_games(&self) -> Result<(), Box<dyn std::error::Error>> {
        let url = format!("{}/gameslist", self.server_url);
        let response = self
            .http_client
            .get(&url)
            .send()
            .await?;

        if response.status().is_success() {
            let games_info: serde_json::Value = response.json().await?;

            if let Some(games) = games_info["games"].as_array() {
                if games.is_empty() {
                    println!("No games found.");
                } else {
                    println!("Available games:");
                    for game in games {
                        if let (Some(game_id), Some(status), Some(start_date)) = (
                            game["game_id"].as_str(),
                            game["status"].as_str(),
                            game["start_date"].as_str()
                        ) {
                            // Only show non-closed games
                            if status != "Closed" {
                                println!("  {game_id} - {status} (created: {start_date})");
                            }
                        }
                    }
                }
            } else {
                println!("Invalid response format from games list endpoint.");
            }
            Ok(())
        } else {
            let error_response: ErrorResponse = response.json().await?;
            Err(format!("Failed to list games: {}", error_response.error).into())
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

    // Parse command line arguments
    let args = Args::parse();

    // Load client configuration
    let config = ClientConfig::load_or_default();
    let server_url = config.server_url();

    // Determine client name from args or config
    let client_name = args.name.unwrap_or_else(|| config.client_name.clone());

    // Create client
    let mut client = TombolaClient::new(&client_name, &server_url);

    // Handle list games request
    if args.listgames {
        match client.list_games().await {
            Ok(()) => {
                std::process::exit(0);
            }
            Err(e) => {
                eprintln!("❌ Failed to list games: {e}");
                std::process::exit(1);
            }
        }
    }

    // Determine game_id
    let game_id = if let Some(provided_game_id) = args.gameid {
        provided_game_id
    } else {
        // No game_id provided - show games list first
        match client.list_games().await {
            Ok(()) => {
                println!();
                println!("Please specify a game ID using --gameid <id> to join a specific game.");
                std::process::exit(0);
            },
            Err(e) => {
                eprintln!("❌ Failed to list games: {e}");
                // Fall back to trying to get current running game as before
                match client.get_game_id().await {
                    Ok(game_info) => {
                        // Extract just the game_id from the formatted string
                        if let Some(id) = game_info.split(',').next() {
                            println!("🔄 No games list available, using detected game: {}", id.trim());
                            id.trim().to_string()
                        } else {
                            eprintln!("❌ Failed to extract game ID from response");
                            std::process::exit(1);
                        }
                    },
                    Err(_) => {
                        eprintln!("❌ No game ID provided and no running game found.");
                        eprintln!("Use --gameid <id> to specify a game or --listgames to see available games.");
                        std::process::exit(1);
                    }
                }
            }
        }
    };

    // Set the game ID in the client
    client.set_game_id(game_id.clone());
    println!("🎮 Using game ID: {game_id}");

    // Check for nocard option
    if let Some(nocard_value) = args.nocard {
        client.set_nocard(nocard_value);
        println!("🎴 Will request {nocard_value} cards during registration");
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

            println!("\n🔥 Starting live monitoring (updating every 2 seconds)...");
            println!("📇 Your Cards ({} total)", assigned_cards.len());
            println!("💡 Numbers highlighted in \x1b[1;33myellow\x1b[0m have been extracted from the pouch");
            println!("🛑 Press Ctrl+C to stop monitoring");
            println!("📋 Final achievements summary will be displayed when exiting\n");

            // Get Game ID once (doesn't change during the game)
            let game_id_info = client.get_game_id().await.unwrap_or_else(|_| "Unknown".to_string());

            // Get all card details once (card data doesn't change during the game)
            let mut card_details = Vec::new();
            for card in &assigned_cards {
                match client.get_assigned_card(&card.card_id).await {
                    Ok(card_info) => {
                        card_details.push(card_info);
                    }
                    Err(e) => {
                        println!("❌ Failed to get card details for {}: {}", card.card_id, e);
                        std::process::exit(1);
                    }
                }
            }

            // Main monitoring loop
            loop {
                // Clear screen for better readability
                print!("\x1b[2J\x1b[1;1H");

                // Show timestamp
                println!("🕐 Last update: {}", chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC"));

                // Show Game ID (cached, doesn't change during game)
                println!("🎮 Game ID: {game_id_info}");

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

                for (index, card_info) in card_details.iter().enumerate() {
                    let (is_bingo, _achievements) = print_card_as_table_with_highlights(index + 1, &card_info.card_id, &card_info.card_data, &extracted_numbers, &scorecard);
                    if is_bingo {
                        bingo_cards.push(card_info.card_id.clone());
                    }
                }

                // Show BINGO summary if any cards have BINGO
                if !bingo_cards.is_empty() {
                    println!("\n🏆 \x1b[1;32mCONGRATULATIONS! You have {} BINGO card(s)!\x1b[0m 🏆", bingo_cards.len());
                    for card_id in bingo_cards {
                        println!("   🎉 BINGO with Card ID: {card_id}");
                    }
                }

                if scorecard.published_score == NUMBERSPERCARD {
                    dump_client_achievements(&client, &assigned_cards).await;
                    return; // Exit if BINGO achieved
                }

                // If --exit flag is set, exit after displaying once
                if args.exit {
                    println!("\n🚪 Exiting after displaying game state (--exit flag)");
                    dump_client_achievements(&client, &assigned_cards).await;
                    break;
                }

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

// Function to dump final achievements summary
async fn dump_client_achievements(client: &TombolaClient, assigned_cards: &[AssignedCardInfo]) {
    println!("\n🏁 ===== FINAL ACHIEVEMENTS SUMMARY =====");
    println!("📋 Client: {}", client.client_name);

    // Get final scorecard
    match client.get_scorecard().await {
        Ok(scorecard) => {
            println!("📊 Final Score: {}", scorecard.published_score);

            println!("\n📇 Card Achievements:");
            let mut cards_with_achievements = 0;

            for (index, card) in assigned_cards.iter().enumerate() {
                // Get card details
                match client.get_assigned_card(&card.card_id).await {
                    Ok(_card_info) => {
                        // Find ALL achievements for this card (not just highest)
                        let mut card_achievements = Vec::new();

                        for (score, score_achievements) in scorecard.get_scoremap() {
                            for achievement in score_achievements {
                                if achievement.card_id == card.card_id {
                                    let achievement_text = match score {
                                        2 => "2 in line".to_string(),
                                        3 => "3 in line".to_string(),
                                        4 => "4 in line".to_string(),
                                        5 => "5 in line".to_string(),
                                        x if *x == NUMBERSPERCARD => "🎉 BINGO 🎉".to_string(),
                                        _ => format!("{score} in line"),
                                    };
                                    card_achievements.push((*score, achievement_text));
                                }
                            }
                        }

                        // Sort achievements by score to show them in order
                        card_achievements.sort_by_key(|(score, _)| *score);

                        // Only show cards that have achievements
                        if !card_achievements.is_empty() {
                            println!("   Card {} ({}):", index + 1, card.card_id);
                            for (_, achievement_text) in card_achievements {
                                println!("     ✅ {achievement_text}");
                            }
                            cards_with_achievements += 1;
                        }
                    },
                    Err(e) => {
                        println!("   Card {} ({}): ❌ Could not fetch card details: {}", index + 1, card.card_id, e);
                    }
                }
            }

            // Show summary of cards with achievements
            if cards_with_achievements == 0 {
                println!("   No cards have achievements yet.");
            } else {
                println!("\n   📊 {} out of {} cards have achievements.", cards_with_achievements, assigned_cards.len());
            }
        },
        Err(e) => {
            println!("⚠️  Warning: Could not fetch final scorecard: {e}");
        }
    }

    println!("🏁 ======================================\n");
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

    // Get achievements from server scorecard - only show if relevant to current published score
    let mut achievements_for_this_card = Vec::new();
    let mut is_bingo = false;
    let mut highest_score = 0;
    let published_score = scorecard.published_score;

    // Check all scores in the scorecard for this card and find the highest
    for (score, score_achievements) in scorecard.get_scoremap() {
        for achievement in score_achievements {
            if achievement.card_id == card_id && *score > highest_score {
                highest_score = *score;
                achievements_for_this_card.clear(); // Clear lower achievements

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

    // Only show achievements if they are relevant to the current published score
    // Don't show obsolete achievements (e.g., don't show "2 in line" if published score is 4)
    if highest_score > 0 && published_score > 0 && highest_score < published_score {
        achievements_for_this_card.clear(); // Clear obsolete achievements
    }

    // Display progress information - BINGO announcement removed (shown in main loop summary)

    // Display current achievement (only if relevant and not obsolete)
    if !achievements_for_this_card.is_empty() {
        if let Some(highest_achievement) = achievements_for_this_card.last() {
            println!("🏆 {highest_achievement}");
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
