// src/clients/registration.rs
// Client registration utilities shared between client applications

use std::error::Error;
use std::time::Duration;
use super::common::{RegisterRequest, RegisterResponse};

/// Join a client to a specific game
pub async fn join_client(
    server_url: &str,
    game_id: &str,
    client_name: &str,
    client_type: &str,
    nocard: Option<u32>,
    email: Option<String>,
    http_client: &reqwest::Client
) -> Result<RegisterResponse, Box<dyn Error>> {
    let request = RegisterRequest {
        name: client_name.to_string(),
        client_type: client_type.to_string(),
        nocard,
        email,
    };

    let url = format!("{server_url}/{game_id}/join");
    let response = http_client
        .post(&url)
        .json(&request)
        .send()
        .await?;

    if response.status().is_success() {
        let register_response: RegisterResponse = response.json().await?;
        Ok(register_response)
    } else {
        let error_text = response.text().await?;
        Err(format!("Registration failed: {error_text}").into())
    }
}

/// Register a client globally (without joining a specific game)
#[allow(dead_code)]
pub async fn register_client(
    server_url: &str,
    client_name: &str,
    client_type: &str,
    email: Option<String>,
) -> Result<RegisterResponse, Box<dyn Error>> {
    let request = RegisterRequest {
        name: client_name.to_string(),
        client_type: client_type.to_string(),
        nocard: Some(0), // Not used in global registration
        email,
    };

    let url = format!("{server_url}/register");
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()?;
    
    let response = client
        .post(&url)
        .json(&request)
        .send()
        .await?;

    if response.status().is_success() {
        let register_response: RegisterResponse = response.json().await?;
        Ok(register_response)
    } else {
        let error_text = response.text().await?;
        Err(format!("Global registration failed: {error_text}").into())
    }
}
