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
use tombola::config::ClientConfig;

#[derive(Parser)]
#[command(name = env!("CARGO_BIN_NAME"))]
#[command(about = "Tombola Board Client - Display game state and perform extractions")]
#[command(version = env!("CARGO_PKG_VERSION"))]
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

pub async fn run_client() -> Result<(), Box<dyn Error>> {
    // Load client configuration
    let config = ClientConfig::load_or_default();
    let server_base_url = config.server_url();
    
    // Main game loop
    loop {
        // Retrieve and display current game state
        let board_numbers = get_board_data(&server_base_url).await?;
        
        // Retrieve scorecard data first
        let scorecard_data = get_scoremap(&server_base_url).await?;
        
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
        let pouch_data = get_pouch_data(&server_base_url).await?;
        
        // Display current state with client names resolved
        show_on_terminal_with_client_names(&display_board, &pouch_data, &scorecard_data, &server_base_url).await;

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
                    match extract_number(&server_base_url).await {
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

async fn test_server_connection(server_base_url: &str) -> Result<(), Box<dyn Error>> {
    let url = format!("{}/status", server_base_url);
    let response = reqwest::get(&url).await?;
    
    if response.status().is_success() {
        Ok(())
    } else {
        Err(format!("Server returned status: {}", response.status()).into())
    }
}

async fn get_board_data(server_base_url: &str) -> Result<Vec<Number>, Box<dyn Error>> {
    let url = format!("{}/board", server_base_url);
    let response = reqwest::get(&url).await?;
    
    if response.status().is_success() {
        let board: Board = response.json().await?;
        Ok(board.get_numbers().clone())
    } else {
        Err(format!("Failed to get board data: {}", response.status()).into())
    }
}

async fn get_pouch_data(server_base_url: &str) -> Result<Vec<Number>, Box<dyn Error>> {
    let url = format!("{}/pouch", server_base_url);
    let response = reqwest::get(&url).await?;
    
    if response.status().is_success() {
        let pouch: tombola::pouch::Pouch = response.json().await?;
        println!("Server reports {} numbers remaining in pouch", pouch.len());
        Ok(pouch.numbers)
    } else {
        Err(format!("Server error: {}", response.status()).into())
    }
}

async fn extract_number(server_base_url: &str) -> Result<u8, Box<dyn Error>> {
    let client = reqwest::Client::new();
    let url = format!("{}/extract", server_base_url);
    
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

async fn get_scoremap(server_base_url: &str) -> Result<tombola::score::ScoreCard, Box<dyn Error>> {
    let url = format!("{}/scoremap", server_base_url);
    let response = reqwest::get(&url).await?;
    
    if response.status().is_success() {
        let scorecard: tombola::score::ScoreCard = response.json().await?;
        Ok(scorecard)
    } else {
        Err(format!("Server error: {}", response.status()).into())
    }
}

async fn get_client_name_by_id(server_base_url: &str, client_id: &str) -> Result<String, Box<dyn Error>> {
    // Handle special board client ID
    if client_id == "0000000000000000" {
        return Ok("Board".to_string());
    }
    
    let url = format!("{}/clientbyid/{}", server_base_url, client_id);
    let response = reqwest::get(&url).await?;
    
    if response.status().is_success() {
        let client_info: serde_json::Value = response.json().await?;
        if let Some(name) = client_info["name"].as_str() {
            Ok(name.to_string())
        } else {
            Ok(format!("Unknown({})", client_id))
        }
    } else {
        // Fallback to showing the client ID if lookup fails
        Ok(format!("ID:{}", client_id))
    }
}

// Custom terminal display function that resolves client names for achievements
async fn show_on_terminal_with_client_names(
    board: &Board,
    pouch: &[Number],
    scorecard: &tombola::score::ScoreCard,
    server_base_url: &str,
) {
    // Get the last extracted number from the board
    let extracted = board.get_numbers().last().copied().unwrap_or(0);

    println!("Last number: {}{extracted}{}", tombola::defs::Colors::green(), tombola::defs::Colors::reset());
    println!("Previous numbers: {:?}", tombola::terminal::print_last_numbers(board, 3));
    println!("\nCurrent board:");
    tombola::terminal::print_board(board);
    println!();

    // Print scorecard with client names resolved
    if scorecard.published_score >= 2 {
        println!();
        println!("ScoreCard achievements:");
        let mut achievements: Vec<_> = scorecard.score_map.iter().collect();
        achievements.sort_by(|a, b| b.0.cmp(a.0)); // Sort descending by score_idx
        for (score_idx, score_achievements) in achievements {
            // Mark numbers only if scorecard reaches a NEW goal
            match score_idx {
                2 => print!("{}TWO in line{}", tombola::defs::Colors::yellow(), tombola::defs::Colors::reset()),
                3 => print!("{}THREE in line{}", tombola::defs::Colors::yellow(), tombola::defs::Colors::reset()),
                4 => print!("{}FOUR in line{}", tombola::defs::Colors::yellow(), tombola::defs::Colors::reset()),
                5 => print!("{}FIVE in line{}", tombola::defs::Colors::yellow(), tombola::defs::Colors::reset()),
                x if *x == tombola::defs::NUMBERSPERCARD => print!("{}BINGO!!!{}", tombola::defs::Colors::yellow(), tombola::defs::Colors::reset()),
                _ => {} // Handle all other cases (do nothing)
            }
            
            // Display card IDs with resolved client names and their contributing numbers
            print!(" -> ");
            for (i, achievement) in score_achievements.iter().enumerate() {
                if i > 0 { print!(", "); }
                
                // Resolve client name
                let client_name = match get_client_name_by_id(server_base_url, &achievement.client_id).await {
                    Ok(name) => name,
                    Err(_) => format!("ID:{}", achievement.client_id),
                };
                
                if achievement.numbers.is_empty() {
                    print!("{} [{}] (no numbers)", client_name, achievement.card_id);
                } else {
                    print!("{} [{}] (numbers: {:?})", client_name, achievement.card_id, achievement.numbers);
                }
            }
            println!();
        }
    }

    if !pouch.is_empty() { 
        println!("\nRemaining in pouch {}:", pouch.len());
        for &pouch_num in pouch {
            print!("{pouch_num:2} ");
        }
        println!();
    }

    println!();
}

async fn call_newgame(server_base_url: &str) -> Result<(), Box<dyn Error>> {
    let client = reqwest::Client::new();
    let url = format!("{}/newgame", server_base_url);
    
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
    // Clear the screen first for a clean start
    print!("\x1Bc");
    
    // Load client configuration
    let config = ClientConfig::load_or_default();
    let server_base_url = config.server_url();
    
    println!("Tombola Terminal Client");
    print!("Connecting to server at {}...", server_base_url);

    // Test server connectivity first
    match test_server_connection(&server_base_url).await {
        Ok(_) => println!("Ok. ✓"),
        Err(e) => {
            eprintln!("Error. ✗ Failed to connect to server: {e}");
            eprintln!("Make sure the tombola server is running on {}", server_base_url);
            return Err(e);
        }
    }
    println!();

    // Handle newgame option if requested
    if args.newgame {
        match call_newgame(&server_base_url).await {
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
