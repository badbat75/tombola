// src/board_client.rs
// This module provides a terminal client that retrieves board and scorecard data from the API,
// displays it using the terminal functions, and allows interactive number extraction.
// 
// Interactive Controls:
// - ENTER: Extract a number using the /extract API endpoint
// - F5: Refresh screen and re-fetch fresh data from server without extracting
// - ESC: Exit the client application
//
// CLI Options:
// - --newgame: Reset the game state before starting the client

use std::error::Error;
use clap::Parser;
use tombola::defs::Number;
use tombola::board::Board;
use tombola::terminal;

#[derive(Parser)]
#[command(name = "board_client")]
#[command(about = "Tombola Board Client - Display game state and perform extractions")]
#[command(version = "0.1.0")]
struct Args {
    /// Reset the game state before starting the client
    #[arg(long)]
    newgame: bool,
}

// Function to extract numbers from the highest achievement for highlighting
// Only emphasizes board client's achievements, and only if no other client has achieved higher
fn extract_highest_achievement_numbers(scorecard: &tombola::score::ScoreCard) -> Vec<Number> {
    const BOARD_CLIENT_ID: &str = "0000000000000000";
    
    if scorecard.published_score < 2 {
        return Vec::new();
    }
    
    // Look for the highest score achieved by the board client
    let score_map = scorecard.get_scoremap();
    let mut board_client_highest_score = 0;
    let mut board_client_numbers = Vec::new();
    
    // Find the board client's highest achievement
    for (score_level, achievements) in score_map.iter() {
        for achievement in achievements {
            if achievement.card_id == BOARD_CLIENT_ID && *score_level > board_client_highest_score {
                board_client_highest_score = *score_level;
                board_client_numbers = achievement.numbers.clone();
            }
        }
    }
    
    // Only show emphasis if the board client has the globally highest score
    if board_client_highest_score == scorecard.published_score && board_client_highest_score >= 2 {
        board_client_numbers
    } else {
        Vec::new() // No emphasis if another client achieved higher
    }
}

// Client configuration
const SERVER_BASE_URL: &str = "http://127.0.0.1:3000";

pub async fn run_client() -> Result<(), Box<dyn Error>> {
    // Main game loop
    loop {
        // Retrieve and display current game state
        let board_numbers = get_board_data().await?;
        
        // Retrieve scorecard data first
        let scorecard_data = get_scoremap().await?;
        
        // Create a board for display purposes and recreate the proper state
        let mut display_board = Board::new();
        
        // Add all numbers from the API response using push_simple
        for number in board_numbers {
            display_board.push_simple(number);
        }
        
        // Extract numbers to highlight from the highest achievement in the scorecard
        let numbers_to_highlight = extract_highest_achievement_numbers(&scorecard_data);
        
        // Update the board's marked numbers with the highest achievement numbers
        display_board.update_marked_numbers(numbers_to_highlight);
        
        // Retrieve pouch data
        let pouch_data = get_pouch_data().await?;
        
        // Display current state
        terminal::show_on_terminal(&display_board, &pouch_data, &scorecard_data);

        // Check if BINGO has been reached - if so, exit immediately
        if scorecard_data.published_score >= 15 {
            println!("🎉 GAME OVER: BINGO has been reached! 🎉");
            println!("The game has ended. No more numbers can be extracted.");
            break; // Exit the game loop immediately
        }

        // Wait for user input and handle actions
        let should_continue = loop {
            match terminal::wait_for_user_action() {
                terminal::KeyAction::Extract => {
                    // Extract a number
                    match extract_number().await {
                        Ok(extracted) => {
                            println!("Successfully extracted number: {}", extracted);
                            break true; // Continue main loop to refresh display
                        }
                        Err(e) => {
                            let error_msg = e.to_string();
                            if error_msg.contains("BINGO has been reached") {
                                println!("🎉 GAME OVER: BINGO has been reached! 🎉");
                                println!("The game has ended. No more numbers can be extracted.");
                                break false; // Exit the main loop
                            } else {
                                eprintln!("Error extracting number: {}", e);
                                // Continue waiting for user input
                                continue;
                            }
                        }
                    }
                }
                terminal::KeyAction::Refresh => {
                    // Refresh: clear screen and re-fetch fresh data
                    print!("\x1Bc"); // Clear the screen
                    println!("🔄 Refreshing game state...");
                    break true; // Continue main loop to fetch fresh data and redisplay
                }
                terminal::KeyAction::Exit => {
                    println!("Exiting the client.");
                    break false; // Exit the main loop
                }
            }
        };

        if !should_continue {
            break;
        }
    }

    println!("Client execution completed successfully.");
    
    Ok(())
}

async fn test_server_connection() -> Result<(), Box<dyn Error>> {
    let url = format!("{SERVER_BASE_URL}/status");
    let response = reqwest::get(&url).await?;
    
    if response.status().is_success() {
        Ok(())
    } else {
        Err(format!("Server returned status: {}", response.status()).into())
    }
}

async fn get_board_data() -> Result<Vec<Number>, Box<dyn Error>> {
    let url = format!("{SERVER_BASE_URL}/board");
    let response = reqwest::get(&url).await?;
    
    if response.status().is_success() {
        let board: Board = response.json().await?;
        Ok(board.get_numbers().clone())
    } else {
        Err(format!("Failed to get board data: {}", response.status()).into())
    }
}

async fn get_pouch_data() -> Result<Vec<Number>, Box<dyn Error>> {
    let url = format!("{SERVER_BASE_URL}/pouch");
    let response = reqwest::get(&url).await?;
    
    if response.status().is_success() {
        let pouch: tombola::pouch::Pouch = response.json().await?;
        println!("Server reports {} numbers remaining in pouch", pouch.len());
        Ok(pouch.numbers)
    } else {
        Err(format!("Server error: {}", response.status()).into())
    }
}

async fn extract_number() -> Result<u8, Box<dyn Error>> {
    let client = reqwest::Client::new();
    let url = format!("{SERVER_BASE_URL}/extract");
    
    let response = client
        .post(&url)
        .header("X-Client-ID", "0000000000000000") // Board client ID
        .send()
        .await?;
    
    if response.status().is_success() {
        let extract_response: serde_json::Value = response.json().await?;
        if let Some(extracted_number) = extract_response["extracted_number"].as_u64() {
            println!("✓ Extracted number: {}", extracted_number);
            Ok(extracted_number as u8)
        } else {
            Err("Invalid response format from extract endpoint".into())
        }
    } else {
        let status = response.status();
        let error_text = response.text().await?;
        Err(format!("Failed to extract number: {} - {}", status, error_text).into())
    }
}

async fn get_scoremap() -> Result<tombola::score::ScoreCard, Box<dyn Error>> {
    let url = format!("{SERVER_BASE_URL}/scoremap");
    let response = reqwest::get(&url).await?;
    
    if response.status().is_success() {
        let scorecard: tombola::score::ScoreCard = response.json().await?;
        Ok(scorecard)
    } else {
        Err(format!("Server error: {}", response.status()).into())
    }
}

async fn call_newgame() -> Result<(), Box<dyn Error>> {
    let client = reqwest::Client::new();
    let url = format!("{SERVER_BASE_URL}/newgame");
    
    println!("🔄 Initiating new game...");
    
    let response = client
        .post(&url)
        .header("X-Client-ID", "0000000000000000") // Board client ID
        .header("Content-Type", "application/json")
        .send()
        .await?;
    
    if response.status().is_success() {
        let newgame_response: serde_json::Value = response.json().await?;
        
        if let Some(message) = newgame_response["message"].as_str() {
            println!("✓ {}", message);
        }
        
        if let Some(components) = newgame_response["reset_components"].as_array() {
            for component in components {
                if let Some(component_str) = component.as_str() {
                    println!("  - {}", component_str);
                }
            }
        }
        
        println!(); // Add blank line for readability
        Ok(())
    } else {
        let status = response.status();
        let error_text = response.text().await?;
        Err(format!("Failed to reset game: {} - {}", status, error_text).into())
    }
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    
    match run_client_with_args(args).await {
        Ok(_) => {
            println!("Client finished successfully.");
        }
        Err(e) => {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
    }
}

async fn run_client_with_args(args: Args) -> Result<(), Box<dyn Error>> {
    println!("Tombola Terminal Client");
    print!("Connecting to server at {SERVER_BASE_URL}...");

    // Test server connectivity first
    match test_server_connection().await {
        Ok(_) => println!("Ok. ✓"),
        Err(e) => {
            eprintln!("Error. ✗ Failed to connect to server: {e}");
            eprintln!("Make sure the tombola server is running on {SERVER_BASE_URL}");
            return Err(e);
        }
    }
    println!();

    // Handle newgame option if requested
    if args.newgame {
        match call_newgame().await {
            Ok(_) => {
                // Success message already printed by call_newgame()
            }
            Err(e) => {
                eprintln!("Failed to reset game: {}", e);
                eprintln!("Continuing with current game state...");
                println!();
            }
        }
    }

    // Run the main client functionality
    run_client().await
}
