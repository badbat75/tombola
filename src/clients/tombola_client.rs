// src/tombola_client.rs
//
// Terminal-based tombola board client that displays game state and allows interactive number extraction.
// This client uses the centralized client library modules for API communication and shared functionality.
//
// Architecture:
// - Uses api_client module for all HTTP API communication
// - Uses game_utils module for game discovery and management
// - Uses terminal module for display and user interaction
// - Direct calls to centralized modules eliminate code duplication
//
// Interactive Controls:
// - ENTER: Extract a number using the /extract API endpoint
// - F5: Refresh screen and re-fetch fresh data from server without extracting
// - ESC: Exit the client application
//
// CLI Options:
// - --newgame: Create a new game before starting the client
// - --gameid: Specify the game ID to connect to
// - --listgames: List active games and exit

use tombola::clients::terminal;

// Use shared modules from library
use tombola::clients::{game_utils, api_client};

use std::error::Error;
use clap::Parser;
use tombola::defs::Number;
use tombola::board::{Board, BOARD_ID};
use tombola::config::ClientConfig;


#[derive(Parser)]
#[command(name = env!("CARGO_BIN_NAME"))]
#[command(about = "Tombola Board Client - Display game state and perform extractions")]
#[command(version = env!("CARGO_PKG_VERSION"))]
struct Args {
    /// Create a new game before starting the client
    #[arg(long)]
    newgame: bool,

    /// Exit after displaying the current state (no interactive loop)
    #[arg(long)]
    exit: bool,

    /// Game ID to connect to (required unless using --newgame or --listgames)
    #[arg(long)]
    gameid: Option<String>,

    /// List active games and exit
    #[arg(long)]
    listgames: bool,
}

// Function to extract numbers from the highest achievement for highlighting
// Only emphasizes board client's achievements, and only if no other client has achieved higher
fn extract_highest_achievement_numbers(scorecard: &tombola::score::ScoreCard) -> Vec<Number> {
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
            if achievement.card_id == BOARD_ID && *score_level > board_client_highest_score {
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
    // This is kept for backward compatibility but will show games list if no game detected
    // Load client configuration to get server URL and try to get current game
    let config = ClientConfig::load_or_default();
    let server_base_url = config.server_url();

    // Try to show games list first, then fall back to get current running game
    match game_utils::list_games(&server_base_url).await {
        Ok(()) => {
            println!();
            println!("Please specify a game ID using the command-line interface with --gameid <id>");
            Ok(())
        },
        Err(_) => {
            // Fall back to trying to get current running game
            let game_id = match game_utils::get_game_id(&server_base_url).await {
                Ok(game_info) => {
                    if let Some(id) = game_info.split(',').next() {
                        println!("Using detected game: {}", id.trim());
                        id.trim().to_string()
                    } else {
                        return Err("Failed to extract game ID from response".into());
                    }
                },
                Err(e) => {
                    return Err(format!("No running game found: {e}").into());
                }
            };

            run_client_with_game_id(&server_base_url, &game_id).await
        }
    }
}

pub async fn run_client_once() -> Result<(), Box<dyn Error>> {
    // This is kept for backward compatibility but will show games list if no game detected
    let config = ClientConfig::load_or_default();
    let server_base_url = config.server_url();

    // Try to show games list first, then fall back to get current running game
    match game_utils::list_games(&server_base_url).await {
        Ok(()) => {
            println!();
            println!("Please specify a game ID using the command-line interface with --gameid <id>");
            Ok(())
        },
        Err(_) => {
            // Fall back to trying to get current running game
            let game_id = match game_utils::get_game_id(&server_base_url).await {
                Ok(game_info) => {
                    if let Some(id) = game_info.split(',').next() {
                        println!("Using detected game: {}", id.trim());
                        id.trim().to_string()
                    } else {
                        return Err("Failed to extract game ID from response".into());
                    }
                },
                Err(e) => {
                    return Err(format!("No running game found: {e}").into());
                }
            };

            run_client_once_with_game_id(&server_base_url, &game_id).await
        }
    }
}

pub async fn run_client_with_game_id(server_base_url: &str, game_id: &str) -> Result<(), Box<dyn Error>> {
    run_client_with_exit_flag_and_game_id(server_base_url, game_id, false).await
}

pub async fn run_client_once_with_game_id(server_base_url: &str, game_id: &str) -> Result<(), Box<dyn Error>> {
    run_client_with_exit_flag_and_game_id(server_base_url, game_id, true).await
}

pub async fn run_client_with_exit_flag_and_game_id(server_base_url: &str, game_id: &str, exit_after_display: bool) -> Result<(), Box<dyn Error>> {
    // Main game loop
    loop {
        // Retrieve and display current game state
        let board_numbers = api_client::get_board_data(server_base_url, game_id).await?;

        // Retrieve scorecard data first
        let scorecard_data = api_client::get_scoremap(server_base_url, game_id).await?;

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
        let pouch_data = api_client::get_pouch_data(server_base_url, game_id).await?;

        // Display current state with client names resolved
        show_on_terminal_with_client_names(&display_board, &pouch_data, &scorecard_data, server_base_url, game_id).await;

        // Check if BINGO has been reached - if so, exit immediately
        if scorecard_data.published_score >= 15 {
            println!("🎉 GAME OVER: BINGO has been reached! 🎉");
            println!("The game has ended. No more numbers can be extracted.");
            break; // Exit the game loop immediately
        }

        // If exit_after_display is true, exit after displaying the state once
        if exit_after_display {
            println!("State displayed. Exiting as requested.");
            break;
        }

        // Wait for user input and handle actions
        let should_continue = loop {
            match terminal::wait_for_user_action() {
                terminal::KeyAction::Extract => {
                    // Extract a number
                    match api_client::extract_number(server_base_url, game_id, BOARD_ID).await {
                        Ok(_) => {
                            break true; // Continue main loop to refresh display
                        }
                        Err(e) => {
                            let error_msg = e.to_string();
                            if error_msg.contains("BINGO has been reached") {
                                println!("🎉 GAME OVER: BINGO has been reached! 🎉");
                                println!("The game has ended. No more numbers can be extracted.");
                                break false; // Exit the main loop
                            } else {
                                eprintln!("Error extracting number: {e}");
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

// Custom terminal display function that resolves client names for achievements
async fn show_on_terminal_with_client_names(
    board: &Board,
    pouch: &[Number],
    scorecard: &tombola::score::ScoreCard,
    server_base_url: &str,
    game_id: &str,
) {
    // Display Game ID first
    println!("Game ID: {game_id}");
    println!();

    // Get the last extracted number from the board
    let extracted = board.get_numbers().last().copied().unwrap_or(0);

    println!("Last number: {}{extracted}{}", tombola::defs::Colors::green(), tombola::defs::Colors::reset());
    println!("Previous numbers: {:?}", terminal::print_last_numbers(board, 3));
    println!("\nCurrent board:");
    terminal::print_board(board);
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
                let client_name = match api_client::get_client_name_by_id(server_base_url, &achievement.client_id).await {
                    Ok(name) => name,
                    Err(_) => "Unknown Client".to_string(),
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

async fn call_newgame(server_base_url: &str) -> Result<String, Box<dyn Error>> {
    let client = reqwest::Client::new();
    let url = format!("{server_base_url}/newgame");

    println!("🔄 Creating new game...");

    let response = client
        .post(&url)
        .header("X-Client-ID", BOARD_ID) // Board client ID
        .send()
        .await?;

    if response.status().is_success() {
        let newgame_response: serde_json::Value = response.json().await?;

        if let Some(success) = newgame_response["success"].as_bool() {
            if success {
                println!("✓ New game created successfully");
            } else {
                println!("⚠ New game creation response indicates failure");
            }
        }

        let game_id = if let Some(game_id) = newgame_response["game_id"].as_str() {
            println!("  Game ID: {game_id}");
            game_id.to_string()
        } else {
            return Err("Game ID not found in newgame response".into());
        };

        if let Some(created_at) = newgame_response["created_at"].as_str() {
            println!("  Created: {created_at}");
        }

        if let Some(note) = newgame_response["note"].as_str() {
            println!("  Note: {note}");
        }

        println!(); // Add blank line for readability
        Ok(game_id)
    } else {
        let status = response.status();
        let error_text = response.text().await?;
        Err(format!("Failed to create new game: {status} - {error_text}").into())
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
    print!("Connecting to server at {server_base_url}...");

    // Test server connectivity first
    match game_utils::test_server_connection(&server_base_url).await {
        Ok(_) => println!("Ok. ✓"),
        Err(e) => {
            eprintln!("Error. ✗ Failed to connect to server: {e}");
            eprintln!("Make sure the tombola server is running on {server_base_url}");
            return Err(e);
        }
    }
    println!();

    // Handle list games request
    if args.listgames {
        return game_utils::list_games(&server_base_url).await;
    }

    // Determine game_id
    let game_id = if args.newgame {
        // Create new game first
        match call_newgame(&server_base_url).await {
            Ok(new_game_id) => new_game_id,
            Err(e) => {
                eprintln!("Failed to reset game: {e}");
                return Err(e);
            }
        }
    } else if let Some(provided_game_id) = args.gameid {
        provided_game_id
    } else {
        // No game_id provided and not creating new game - show games list first
        match game_utils::list_games(&server_base_url).await {
            Ok(()) => {
                println!();
                println!("Please specify a game ID using --gameid <id> or create a new game with --newgame");
                return Ok(());
            },
            Err(e) => {
                eprintln!("Failed to list games: {e}");
                // Fall back to trying to get current running game as before
                match game_utils::get_game_id(&server_base_url).await {
                    Ok(game_info) => {
                        // Extract just the game_id from the formatted string
                        if let Some(id) = game_info.split(',').next() {
                            println!("No games list available, using detected game: {}", id.trim());
                            id.trim().to_string()
                        } else {
                            return Err("Failed to extract game ID from response".into());
                        }
                    },
                    Err(_) => {
                        eprintln!("No game ID provided and no running game found.");
                        eprintln!("Use --gameid <id> to specify a game, --newgame to create one, or --listgames to see available games.");
                        return Err("Game ID required".into());
                    }
                }
            }
        }
    };

    println!("Using game ID: {game_id}");

    // Run the main client functionality with game_id
    if args.exit {
        run_client_once_with_game_id(&server_base_url, &game_id).await
    } else {
        run_client_with_game_id(&server_base_url, &game_id).await
    }
}
