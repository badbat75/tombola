// src/clients/card_management.rs
// Card generation and management utilities shared between client applications

use std::error::Error;
use super::common::{GenerateCardsRequest, GenerateCardsResponse, ListAssignedCardsResponse, CardInfo, ErrorResponse};

/// Generate cards for a client
pub async fn generate_cards(
    server_url: &str,
    game_id: &str,
    client_id: &str,
    count: u32,
    http_client: &reqwest::Client
) -> Result<GenerateCardsResponse, Box<dyn Error>> {
    let request = GenerateCardsRequest { count };
    let url = format!("{server_url}/{game_id}/generatecardsforme");

    let response = http_client
        .post(&url)
        .header("X-Client-ID", client_id)
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

/// List assigned cards for a client
pub async fn list_assigned_cards(
    server_url: &str,
    game_id: &str,
    client_id: &str,
    http_client: &reqwest::Client
) -> Result<ListAssignedCardsResponse, Box<dyn Error>> {
    let url = format!("{server_url}/{game_id}/listassignedcards");
    let response = http_client
        .get(&url)
        .header("X-Client-ID", client_id)
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
pub async fn get_assigned_card(
    server_url: &str,
    game_id: &str,
    client_id: &str,
    card_id: &str,
    http_client: &reqwest::Client
) -> Result<CardInfo, Box<dyn Error>> {
    let url = format!("{server_url}/{game_id}/getassignedcard/{card_id}");
    let response = http_client
        .get(&url)
        .header("X-Client-ID", client_id)
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
