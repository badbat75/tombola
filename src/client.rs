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
            println!("Client already registered with ID: {}", self.client_id.as_ref().unwrap());
            return Ok(());
        }

        let register_request = RegisterRequest {
            name: self.client_name.clone(),
            client_type: "tombola_client".to_string(),
            nocard: self.nocard, // Include nocard in the registration request
        };

        let url = format!("{}/register", self.server_url);
        println!("Registering client '{}' with server...", self.client_name);

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
    if args.iter().any(|arg| arg == "--nocard") {
        client.set_nocard(1);
        println!("🎴 Will request 1 card during registration");
    }
    
    // Register with server
    match client.register().await {
        Ok(()) => {
            println!("✅ Client registered successfully!");
            
            // Try to generate cards (will fail if --nocard was used)
            let _ = client.generate_cards(3).await;
            
            // Get and display all assigned cards
            match client.list_assigned_cards().await {
                Ok(response) => {
                    if response.cards.is_empty() {
                        println!("No cards assigned to this client.");
                    } else {
                        println!("\n📇 Your Cards ({} total):", response.cards.len());
                        
                        for (index, card) in response.cards.iter().enumerate() {
                            match client.get_assigned_card(&card.card_id).await {
                                Ok(card_info) => {
                                    print_card_as_table(index + 1, &card_info.card_id, &card_info.card_data);
                                }
                                Err(e) => {
                                    println!("   ❌ Failed to get card {} details: {}", index + 1, e);
                                }
                            }
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

fn print_card_as_table(card_number: usize, card_id: &str, card_data: &Vec<Vec<Option<u8>>>) {
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
    
    for row in card_data {
        print!("│");
        for cell in row {
            match cell {
                Some(number) => print!("   {:2}   │", number),
                None => print!("        │"),
            }
        }
        println!();
    }
    
    println!("└────────┴────────┴────────┴────────┴────────┴────────┴────────┴────────┴────────┘");
}
