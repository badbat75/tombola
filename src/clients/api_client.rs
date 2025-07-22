// src/clients/api_client.rs
// HTTP API client utilities for tombola game communication

use crate::board::Board;
use crate::score::ScoreCard;
use crate::pouch::Pouch;
use crate::defs::Number;
use std::error::Error;
use super::common::{get_json, get_json_with_client_id, post_json_with_client_id};

// ============================================================================
// Game State API Calls
// ============================================================================

/// Get board data from the server
pub async fn get_board_data(server_base_url: &str, game_id: &str) -> Result<Vec<Number>, Box<dyn Error>> {
    let url = format!("{server_base_url}/{game_id}/board");
    let board: Board = get_json(&url).await?;
    Ok(board.get_numbers().clone())
}

/// Get pouch data from the server
pub async fn get_pouch_data(server_base_url: &str, game_id: &str) -> Result<Vec<Number>, Box<dyn Error>> {
    let url = format!("{server_base_url}/{game_id}/pouch");
    let pouch: Pouch = get_json(&url).await?;
    Ok(pouch.numbers)
}

/// Get scorecard/scoremap from the server
pub async fn get_scoremap(server_base_url: &str, game_id: &str) -> Result<ScoreCard, Box<dyn Error>> {
    let url = format!("{server_base_url}/{game_id}/scoremap");
    get_json(&url).await
}

/// Extract a number (requires board client ID)
pub async fn extract_number(server_base_url: &str, game_id: &str, client_id: &str) -> Result<u8, Box<dyn Error>> {
    let url = format!("{server_base_url}/{game_id}/extract");
    let response = post_json_with_client_id::<(), serde_json::Value>(&url, &(), client_id).await?;

    if let Some(extracted_number) = response["extracted_number"].as_u64() {
        Ok(extracted_number as u8)
    } else {
        Err("Invalid response format from extract endpoint".into())
    }
}

/// Get client name by ID
pub async fn get_client_name_by_id(server_base_url: &str, client_id: &str) -> Result<String, Box<dyn Error>> {
    // Handle special board client ID
    if client_id == crate::board::BOARD_ID {
        return Ok("Board".to_string());
    }

    let url = format!("{server_base_url}/clientinfo/{client_id}");
    let client_info: serde_json::Value = get_json(&url).await?;

    if let Some(name) = client_info["name"].as_str() {
        Ok(name.to_string())
    } else {
        Ok("Unknown Client".to_string())
    }
}

// ============================================================================
// Player-specific API Calls
// ============================================================================

/// Get server status for a specific game
pub async fn get_game_status(server_base_url: &str, game_id: &str, client_id: &str) -> Result<serde_json::Value, Box<dyn Error>> {
    let url = format!("{server_base_url}/{game_id}/status");
    get_json_with_client_id(&url, client_id).await
}

/// Get board data with client authentication
pub async fn get_board_with_auth(server_base_url: &str, game_id: &str, client_id: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    let url = format!("{server_base_url}/{game_id}/board");
    let board: Board = get_json_with_client_id(&url, client_id).await?;
    Ok(board.get_numbers().clone())
}

/// Get scorecard with client authentication
pub async fn get_scorecard_with_auth(server_base_url: &str, game_id: &str, client_id: &str) -> Result<ScoreCard, Box<dyn Error>> {
    let url = format!("{server_base_url}/{game_id}/scoremap");
    get_json_with_client_id(&url, client_id).await
}

/// Get pouch state with client authentication
pub async fn get_pouch_with_auth(server_base_url: &str, game_id: &str, client_id: &str) -> Result<(Vec<u8>, usize), Box<dyn Error>> {
    let url = format!("{server_base_url}/{game_id}/pouch");
    let pouch: Pouch = get_json_with_client_id(&url, client_id).await?;
    let remaining_count = pouch.len();
    Ok((pouch.numbers, remaining_count))
}
