// src/clients/common.rs
// Common data structures and utilities shared between client applications

use serde::{Deserialize, Serialize};
use std::error::Error;
use std::time::Duration;

// ============================================================================
// Common Request/Response Structures
// ============================================================================

/// Client registration request
#[derive(Debug, Serialize)]
pub struct RegisterRequest {
    pub name: String,
    pub client_type: String,
    pub nocard: Option<u32>,  // Number of cards to generate during registration
    pub email: Option<String>,  // Optional email for registration
}

/// Client registration response
#[derive(Debug, Deserialize)]
pub struct RegisterResponse {
    pub client_id: String,
    pub message: String,
}

/// Generic API error response structure
#[derive(Debug, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
}

/// Card generation request
#[derive(Debug, Serialize)]
pub struct GenerateCardsRequest {
    pub count: u32,
}

/// Card generation response
#[derive(Debug, Deserialize)]
pub struct GenerateCardsResponse {
    pub cards: Vec<CardInfo>,
    pub message: String,
}

/// Card info structure
#[derive(Debug, Deserialize)]
pub struct CardInfo {
    pub card_id: String,
    pub card_data: Vec<Vec<Option<u8>>>, // 2D vector of optional u8 values
}

/// List assigned cards response
#[derive(Debug, Deserialize)]
pub struct ListAssignedCardsResponse {
    pub cards: Vec<AssignedCardInfo>,
}

/// Assigned card info
#[derive(Debug, Deserialize)]
pub struct AssignedCardInfo {
    pub card_id: String,
    pub assigned_to: String,
}

// ============================================================================
// HTTP Client Utilities
// ============================================================================

/// Simple HTTP GET request wrapper
pub async fn get_json<T>(url: &str) -> Result<T, Box<dyn Error>>
where
    T: for<'de> Deserialize<'de>,
{
    let response = reqwest::get(url).await?;

    if response.status().is_success() {
        Ok(response.json().await?)
    } else {
        Err(format!("HTTP request failed with status: {}", response.status()).into())
    }
}

/// HTTP GET request with client ID header
pub async fn get_json_with_client_id<T>(url: &str, client_id: &str) -> Result<T, Box<dyn Error>>
where
    T: for<'de> Deserialize<'de>,
{
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()?;
    let response = client
        .get(url)
        .header("X-Client-ID", client_id)
        .send()
        .await?;

    if response.status().is_success() {
        Ok(response.json().await?)
    } else {
        Err(format!("HTTP request failed with status: {}", response.status()).into())
    }
}

/// HTTP POST request with JSON body and client ID header
pub async fn post_json_with_client_id<T, U>(url: &str, body: &T, client_id: &str) -> Result<U, Box<dyn Error>>
where
    T: Serialize,
    U: for<'de> Deserialize<'de>,
{
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()?;
    let response = client
        .post(url)
        .json(body)
        .header("X-Client-ID", client_id)
        .send()
        .await?;

    if response.status().is_success() {
        Ok(response.json().await?)
    } else {
        Err(format!("HTTP request failed with status: {}", response.status()).into())
    }
}
