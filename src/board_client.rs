// src/client.rs
// This module provides a terminal client that retrieves board and scorecard data from the API
// and displays it using the terminal functions.

use std::error::Error;
use tombola::defs::Number;
use tombola::board::Board;
use tombola::terminal;

// Client configuration
const SERVER_BASE_URL: &str = "http://127.0.0.1:3000";

pub async fn run_client() -> Result<(), Box<dyn Error>> {
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

    // Retrieve board data
    let board_numbers = get_board_data().await?;
    // Create a board for display purposes and let it calculate the score automatically
    let mut display_board = Board::new();
    
    // Add all numbers from the API response using push (this will automatically calculate score)
    for number in board_numbers {
        display_board.push(number);
    }
    
    // Retrieve pouch data
    let pouch_data = get_pouch_data().await?;
    
    // Retrieve scorecard data
    let scorecard_data = get_scoremap().await?;
    
    terminal::show_on_terminal(&display_board, &pouch_data, &scorecard_data);

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
        println!("Server reports {} numbers remaining in pouch", pouch.remaining);
        Ok(pouch.numbers)
    } else {
        Err(format!("Server error: {}", response.status()).into())
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

#[tokio::main]
async fn main() {
    match run_client().await {
        Ok(_) => {
            println!("Client finished successfully.");
        }
        Err(e) => {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
    }
}
