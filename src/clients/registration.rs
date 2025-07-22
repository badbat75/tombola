// src/clients/registration.rs
// Client registration utilities shared between client applications

use std::error::Error;
use super::common::{RegisterRequest, RegisterResponse};

/// Register a client with the server
pub async fn register_client(
    server_url: &str,
    game_id: &str,
    client_name: &str,
    client_type: &str,
    nocard: Option<u32>,
    http_client: &reqwest::Client
) -> Result<RegisterResponse, Box<dyn Error>> {
    let request = RegisterRequest {
        name: client_name.to_string(),
        client_type: client_type.to_string(),
        nocard,
    };

    let url = format!("{server_url}/{game_id}/register");
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
