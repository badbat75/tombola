// src/clients/game_utils.rs
// Game management utilities shared between client applications

use std::error::Error;
use super::common::get_json;

// ============================================================================
// Game Discovery and Management
// ============================================================================

/// Get the first available running game ID with creation info
pub async fn get_game_id(server_base_url: &str) -> Result<String, Box<dyn Error>> {
    let url = format!("{server_base_url}/gameslist");
    let games_info: serde_json::Value = get_json(&url).await?;

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
}

/// List all available games
pub async fn list_games(server_base_url: &str) -> Result<(), Box<dyn Error>> {
    let url = format!("{server_base_url}/gameslist");
    let games_info: serde_json::Value = get_json(&url).await?;

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
}

/// Test server connection
pub async fn test_server_connection(server_base_url: &str) -> Result<(), Box<dyn Error>> {
    let url = format!("{server_base_url}/gameslist");
    let response = reqwest::get(&url).await?;

    if response.status().is_success() {
        Ok(())
    } else {
        Err(format!("Server returned status: {}", response.status()).into())
    }
}

/// Extract game ID from formatted string (used when `get_game_id` returns "`game_id`, started at: date")
#[must_use] pub fn extract_game_id_from_info(game_info: &str) -> Option<String> {
    game_info.split(',').next().map(|id| id.trim().to_string())
}

/// Common game discovery pattern used by both clients
pub async fn discover_game_id(server_base_url: &str, provided_game_id: Option<String>) -> Result<String, Box<dyn Error>> {
    if let Some(game_id) = provided_game_id {
        return Ok(game_id);
    }

    // No game_id provided - show games list first
    match list_games(server_base_url).await {
        Ok(()) => {
            println!();
            println!("Please specify a game ID using --gameid <id> to join a specific game.");
            std::process::exit(0);
        },
        Err(_) => {
            // Fall back to trying to get current running game
            match get_game_id(server_base_url).await {
                Ok(game_info) => {
                    if let Some(id) = extract_game_id_from_info(&game_info) {
                        println!("🔄 No games list available, using detected game: {id}");
                        Ok(id)
                    } else {
                        Err("Failed to extract game ID from response".into())
                    }
                },
                Err(_) => {
                    Err("No game ID provided and no running game found. Use --gameid <id> to specify a game or --listgames to see available games.".into())
                }
            }
        }
    }
}
