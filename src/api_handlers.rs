use std::sync::Arc;
use std::time::UNIX_EPOCH;

use axum::{
    extract::{State, Query, Path},
    http::{StatusCode, HeaderMap},
    response::{Json, IntoResponse, Response},
    Json as JsonExtractor,
};
use serde::{Deserialize};
use serde_json::json;

use crate::client::{RegisterRequest, RegisterResponse, ClientInfoResponse, ClientInfo};
use crate::card::{ListAssignedCardsResponse, AssignedCardInfo, GenerateCardsRequest, GenerateCardsResponse};
use crate::board::{Board, BOARD_ID};
use crate::pouch::Pouch;
use crate::score::ScoreCard;
use crate::logging::{log_info, log_error, log_warning, log_error_stderr};
use crate::server::AppState;

// Response structures for JSON serialization
#[derive(serde::Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

// Custom error type for handlers
#[derive(Debug)]
pub struct ApiError {
    pub status: StatusCode,
    pub message: String,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let error_response = ErrorResponse {
            error: self.message,
        };
        (self.status, Json(error_response)).into_response()
    }
}

impl ApiError {
    pub fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }
}

#[derive(Deserialize)]
pub struct ClientIdQuery {
    pub client_id: Option<String>,
}

#[derive(Deserialize)]
pub struct ClientNameQuery {
    pub name: Option<String>,
}

#[derive(Deserialize)]
pub struct CardQuery {
    pub client_id: Option<String>,
    pub card_id: Option<String>,
}

#[derive(Deserialize)]
pub struct DumpGameQuery {
    pub filename: Option<String>,
}

pub async fn handle_register(
    State(app_state): State<Arc<AppState>>,
    JsonExtractor(request): JsonExtractor<RegisterRequest>,
) -> Result<Json<RegisterResponse>, ApiError> {
    log_info(&format!("Client registration request: {request:?}"));

    let client_name = &request.name;
    let client_type = &request.client_type;

    // Create client info first
    let client_info = ClientInfo::new(
        client_name,
        client_type,
    );
    let client_id = client_info.id.clone();

    // Check if client already exists and return existing info
    if let Ok(mut registry) = app_state.game.client_registry().lock() {
        if let Some(existing_client) = registry.get(client_name) {
            return Ok(Json(RegisterResponse {
                client_id: existing_client.id.clone(),
                message: format!("Client '{client_name}' already registered"),
            }));
        }

        // Check if numbers have been extracted using the game's convenience method
        let numbers_extracted = app_state.game.has_game_started();

        // Try to register the new client (will fail if numbers have been extracted)
        match registry.insert(client_name.to_string(), client_info, numbers_extracted) {
            Ok(_) => {
                log_info(&format!("Client registered successfully: {client_id}"));
            }
            Err(e) => {
                log_error(&format!("Failed to register client: {e}"));
                return Err(ApiError::new(StatusCode::CONFLICT, e));
            }
        }
    } else {
        log_error("Failed to access client registry");
        return Err(ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "Failed to access client registry"));
    }

    // Check if client requested cards during registration, default to 1 if not specified
    let card_count = request.nocard.unwrap_or(1);
            log_info(&format!("Generating {card_count} cards for client '{client_name}' during registration"));    // Generate the requested number of cards using the card manager
    if let Ok(mut manager) = app_state.game.card_manager().lock() {
        manager.assign_cards(&client_id, card_count);
        log_info(&format!("Generated and assigned {card_count} cards to client '{client_name}'"));
    } else {
        log_warning(&format!("Failed to acquire card manager lock for client '{client_name}'"));
    }

    Ok(Json(RegisterResponse {
        client_id,
        message: format!("Client '{client_name}' registered successfully"),
    }))
}

pub async fn handle_client_info(
    State(app_state): State<Arc<AppState>>,
    Query(params): Query<ClientNameQuery>,
) -> Result<Json<ClientInfoResponse>, ApiError> {
    let client_name = params.name.unwrap_or_default();
    log_info(&format!("Client info request for: {client_name}"));

    if let Ok(registry) = app_state.game.client_registry().lock() {
        if let Some(client) = registry.get(&client_name) {
            Ok(Json(ClientInfoResponse {
                client_id: client.id.clone(),
                name: client.name.clone(),
                client_type: client.client_type.clone(),
                registered_at: format!("{:?}", client.registered_at),
            }))
        } else {
            Err(ApiError::new(StatusCode::NOT_FOUND, format!("Client '{client_name}' not found")))
        }
    } else {
        Err(ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "Failed to access client registry"))
    }
}

pub async fn handle_client_info_by_id(
    State(app_state): State<Arc<AppState>>,
    Path(client_id): Path<String>,
) -> Result<Json<ClientInfoResponse>, ApiError> {
    log_info(&format!("Client info by ID request for: {client_id}"));

    // Use ClientRegistry method to resolve client name (handles both special board case and regular clients)
    let client_name = if let Ok(registry) = app_state.game.client_registry().lock() {
        registry.get_client_name_by_id(&client_id)
    } else {
        None
    }.unwrap_or_else(|| "Unknown".to_string());

    // Handle special case for board client ID
    if client_name == "Board" {
        return Ok(Json(ClientInfoResponse {
            client_id: client_id.clone(),
            name: "Board".to_string(),
            client_type: "board".to_string(),
            registered_at: "System".to_string(),
        }));
    }

    // Handle regular clients - if CardAssignmentManager found a name, look up full client info
    if client_name != "Unknown" {
        if let Ok(registry) = app_state.game.client_registry().lock() {
            for client_info in registry.values() {
                if client_info.name == client_name {
                    return Ok(Json(ClientInfoResponse {
                        client_id: client_info.id.clone(),
                        name: client_info.name.clone(),
                        client_type: client_info.client_type.clone(),
                        registered_at: format!("{:?}", client_info.registered_at),
                    }));
                }
            }
        }
    }

    // Client not found
    Err(ApiError::new(StatusCode::NOT_FOUND, format!("Client with ID '{client_id}' not found")))
}

pub async fn handle_generate_cards(
    State(app_state): State<Arc<AppState>>,
    headers: HeaderMap,
    JsonExtractor(request): JsonExtractor<GenerateCardsRequest>,
) -> Result<Json<GenerateCardsResponse>, ApiError> {
    log_info("Generate cards request");

    // Get client ID from headers
    let client_id = match headers.get("X-Client-ID") {
        Some(header_value) => {
            match header_value.to_str() {
                Ok(id) => id.to_string(),
                Err(_) => {
                    log_error("Invalid client ID in header");
                    return Err(ApiError::new(StatusCode::BAD_REQUEST, "Invalid client ID in header"));
                }
            }
        }
        None => {
            log_error("Client ID header (X-Client-ID) is required");
            return Err(ApiError::new(StatusCode::BAD_REQUEST, "Client ID header (X-Client-ID) is required"));
        }
    };

    // Verify client is registered
    let client_exists = if let Ok(registry) = app_state.game.client_registry().lock() {
        registry.values().any(|client| client.id == client_id)
    } else {
        false
    };

    if !client_exists {
        log_error("Client not registered");
        return Err(ApiError::new(StatusCode::UNAUTHORIZED, "Client not registered"));
    }

    // Check if client already has cards assigned (prevent duplicate generation)
    if let Ok(manager) = app_state.game.card_manager().lock() {
        if let Some(existing_cards) = manager.get_client_cards(&client_id) {
            if !existing_cards.is_empty() {
                log_error("Client already has cards assigned. Card generation is only allowed during registration.");
                return Err(ApiError::new(StatusCode::CONFLICT, "Client already has cards assigned. Card generation is only allowed during registration."));
            }
        }
    }

    // Generate cards using the CardAssignmentManager
    let card_infos = if let Ok(mut manager) = app_state.game.card_manager().lock() {
        let (cards, _) = manager.assign_cards(&client_id, request.count);
        cards
    } else {
        log_error("Failed to acquire card manager lock");
        return Err(ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "Failed to acquire card manager lock"));
    };

    log_info(&format!("Generated {} cards for client {}", card_infos.len(), client_id));

    // Create response
    let response = GenerateCardsResponse {
        cards: card_infos,
        message: format!("Generated {} cards successfully", request.count),
    };

    Ok(Json(response))
}

pub async fn handle_list_assigned_cards(
    State(app_state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(_params): Query<ClientIdQuery>,
) -> Result<Json<ListAssignedCardsResponse>, ApiError> {
    log_info("List assigned cards request");

    // Get client ID from headers
    let client_id = match headers.get("X-Client-ID") {
        Some(header_value) => {
            match header_value.to_str() {
                Ok(id) => id.to_string(),
                Err(_) => {
                    log_error("Invalid client ID in header");
                    return Err(ApiError::new(StatusCode::BAD_REQUEST, "Invalid client ID in header"));
                }
            }
        }
        None => {
            log_error("Client ID header (X-Client-ID) is required");
            return Err(ApiError::new(StatusCode::BAD_REQUEST, "Client ID header (X-Client-ID) is required"));
        }
    };

    // Verify client is registered
    let client_exists = if let Ok(registry) = app_state.game.client_registry().lock() {
        registry.values().any(|client| client.id == client_id)
    } else {
        false
    };

    if !client_exists {
        log_error("Client not registered");
        return Err(ApiError::new(StatusCode::UNAUTHORIZED, "Client not registered"));
    }

    // Get client's assigned cards
    let assigned_cards = if let Ok(manager) = app_state.game.card_manager().lock() {
        manager.get_client_cards(&client_id).cloned().unwrap_or_default()
    } else {
        Vec::new()
    };

    // Create response
    let card_infos: Vec<AssignedCardInfo> = assigned_cards.iter().map(|card_id| {
        AssignedCardInfo {
            card_id: card_id.clone(), // Clone needed for owned response
            assigned_to: client_id.clone(), // Clone needed since used multiple times
        }
    }).collect();

    let response = ListAssignedCardsResponse {
        cards: card_infos,
    };

    Ok(Json(response))
}

pub async fn handle_get_assigned_card(
    State(app_state): State<Arc<AppState>>,
    Path(card_id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    log_info(&format!("Get assigned card request for card ID: {card_id}"));

    // Get client ID from headers
    let client_id = match headers.get("X-Client-ID") {
        Some(header_value) => {
            match header_value.to_str() {
                Ok(id) => id.to_string(),
                Err(_) => {
                    log_error("Invalid client ID in header");
                    return Err(ApiError::new(StatusCode::BAD_REQUEST, "Invalid client ID in header"));
                }
            }
        }
        None => {
            log_error("Client ID header (X-Client-ID) is required");
            return Err(ApiError::new(StatusCode::BAD_REQUEST, "Client ID header (X-Client-ID) is required"));
        }
    };

    // Verify client is registered
    let client_exists = if let Ok(registry) = app_state.game.client_registry().lock() {
        registry.values().any(|client| client.id == client_id)
    } else {
        false
    };

    if !client_exists {
        log_error(&format!("Client not registered: {client_id}"));
        return Err(ApiError::new(StatusCode::UNAUTHORIZED, "Client not registered"));
    }

    // Get the card assignment
    let card_assignment = if let Ok(manager) = app_state.game.card_manager().lock() {
        manager.get_card_assignment(&card_id).cloned()
    } else {
        None
    };

    // Verify the card exists and belongs to the client
    let card_assignment = match card_assignment {
        Some(assignment) => {
            if assignment.client_id != client_id {
                log_error(&format!("Card {card_id} not assigned to client {client_id}"));
                return Err(ApiError::new(StatusCode::FORBIDDEN, "Card not assigned to this client"));
            }
            assignment
        }
        None => {
            log_error(&format!("Card not found: {card_id}"));
            return Err(ApiError::new(StatusCode::NOT_FOUND, "Card not found"));
        }
    };

    // Create response
    let card_info = crate::card::CardInfo {
        card_id: card_assignment.card_id,
        card_data: card_assignment.card_data.iter().map(|row| {
            row.to_vec()
        }).collect(),
    };

    Ok(Json(serde_json::to_value(&card_info).unwrap()))
}

pub async fn handle_board(
    State(app_state): State<Arc<AppState>>,
    Query(_params): Query<ClientIdQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    log_info("Board request");

    let board_data = if let Ok(board) = app_state.game.board().lock() {
        serde_json::to_value(&*board).unwrap_or_else(|_| serde_json::json!({}))
    } else {
        serde_json::to_value(Board::new()).unwrap_or_else(|_| serde_json::json!({}))
    };

    Ok(Json(board_data))
}

pub async fn handle_pouch(
    State(app_state): State<Arc<AppState>>,
    Query(_params): Query<ClientIdQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    log_info("Pouch request");

    let pouch_data = if let Ok(pouch) = app_state.game.pouch().lock() {
        serde_json::to_value(&*pouch).unwrap_or_else(|_| serde_json::json!({}))
    } else {
        serde_json::to_value(Pouch::new()).unwrap_or_else(|_| serde_json::json!({}))
    };

    Ok(Json(pouch_data))
}

pub async fn handle_scoremap(
    State(app_state): State<Arc<AppState>>,
    Query(_params): Query<ClientIdQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    log_info("Score map request");

    let scorecard_data = if let Ok(scorecard) = app_state.game.scorecard().lock() {
        serde_json::to_value(&*scorecard).unwrap_or_else(|_| serde_json::json!({}))
    } else {
        serde_json::to_value(ScoreCard::new()).unwrap_or_else(|_| serde_json::json!({}))
    };

    Ok(Json(scorecard_data))
}

pub async fn handle_status(
    State(app_state): State<Arc<AppState>>,
    Query(_params): Query<ClientIdQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    log_info("Status request");

    let board_len = app_state.game.board_length();
    let scorecard = app_state.game.published_score();

    Ok(Json(json!({
        "status": "running",
        "game_id": app_state.game.id(),
        "created_at": app_state.game.created_at_string(),
        "numbers_extracted": board_len,
        "scorecard": scorecard,
        "server": "axum"
    })))
}

pub async fn handle_running_game_id(
    State(app_state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, ApiError> {
    log_info("Running game ID request");

    let (game_id, created_at_string, created_at_systemtime) = app_state.game.get_running_game_info();
    let created_at_timestamp = created_at_systemtime
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    Ok(Json(json!({
        "game_id": game_id,
        "created_at": created_at_string,
        "created_at_timestamp": {
            "secs_since_epoch": created_at_timestamp,
            "nanos_since_epoch": created_at_systemtime.duration_since(UNIX_EPOCH)
                .unwrap_or_default().subsec_nanos()
        }
    })))
}

pub async fn handle_extract(
    State(app_state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(_params): Query<ClientIdQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    log_info("Extract request");

    // Get client ID from headers for authentication
    let client_id = match headers.get("X-Client-ID") {
        Some(header_value) => {
            match header_value.to_str() {
                Ok(id) => id.to_string(),
                Err(_) => {
                    log_error("Invalid client ID in header");
                    return Err(ApiError::new(StatusCode::BAD_REQUEST, "Invalid client ID in header"));
                }
            }
        }
        None => {
            log_error("Client ID header (X-Client-ID) is required");
            return Err(ApiError::new(StatusCode::BAD_REQUEST, "Client ID header (X-Client-ID) is required"));
        }
    };

    // Only allow board client (ID: "0000000000000000") to extract numbers
    if client_id != BOARD_ID {
        log_error("Unauthorized: Only board client can extract numbers");
        return Err(ApiError::new(StatusCode::FORBIDDEN, "Unauthorized: Only board client can extract numbers"));
    }

    // Check if BINGO has been reached - if so, no more extractions allowed
    if app_state.game.is_bingo_reached() {
        return Err(ApiError::new(StatusCode::CONFLICT, "Game over: BINGO has been reached. No more numbers can be extracted."));
    }

    // Extract a number using the game's coordinated extraction logic
    match app_state.game.extract_number(0) {
        Ok((extracted_number, _new_working_score)) => {
            // Get current pouch and board state for response using Game methods
            let numbers_remaining = app_state.game.pouch_length();
            let total_extracted = app_state.game.board_length();

            // Check if BINGO was reached after this extraction and dump game state if so
            if app_state.game.is_bingo_reached() {
                match app_state.game.dump_to_json() {
                    Ok(dump_message) => {
                        log_info(&format!("Game ended with BINGO! {dump_message}"));
                    }
                    Err(dump_error) => {
                        log_error(&format!("Failed to dump game state: {dump_error}"));
                    }
                }
            }

            Ok(Json(json!({
                "success": true,
                "extracted_number": extracted_number,
                "numbers_remaining": numbers_remaining,
                "total_extracted": total_extracted,
                "message": format!("Number {} extracted successfully", extracted_number)
            })))
        }
        Err(error_msg) => {
            // Handle extraction errors - match old behavior with proper status codes
            log_error(&format!("Failed to extract number: {error_msg}"));
            if error_msg.contains("empty") {
                Err(ApiError::new(StatusCode::CONFLICT, error_msg))
            } else {
                Err(ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, error_msg))
            }
        }
    }
}

pub async fn handle_newgame(
    State(app_state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    log_info("New game request");

    // Get client ID from headers for authentication
    let client_id = match headers.get("X-Client-ID") {
        Some(header_value) => {
            match header_value.to_str() {
                Ok(id) => id.to_string(),
                Err(_) => {
                    log_error("Invalid client ID in header");
                    return Err(ApiError::new(StatusCode::BAD_REQUEST, "Invalid client ID in header"));
                }
            }
        }
        None => {
            log_error("Client ID header (X-Client-ID) is required");
            return Err(ApiError::new(StatusCode::BAD_REQUEST, "Client ID header (X-Client-ID) is required"));
        }
    };

    // Only allow board client (ID: "0000000000000000") to reset the game
    if client_id != BOARD_ID {
        log_error("Unauthorized: Only board client can reset the game");
        return Err(ApiError::new(StatusCode::FORBIDDEN, "Unauthorized: Only board client can reset the game"));
    }

    // Dump the current game state only if the game has started but BINGO was not reached
    // (BINGO games are already auto-dumped when BINGO occurs)
    if app_state.game.has_game_started() && !app_state.game.is_bingo_reached() {
        match app_state.game.dump_to_json() {
            Ok(dump_message) => {
                log_info(&format!("Incomplete game dumped before reset: {dump_message}"));
            }
            Err(dump_error) => {
                log_error(&format!("Failed to dump incomplete game state before reset: {dump_error}"));
            }
        }
    }

    // Use the Game struct's reset_game method which handles proper mutex coordination
    match app_state.game.reset_game() {
        Ok(reset_components) => {
            log_info(&format!("Game reset successful for {}", app_state.game.game_info()));
            // Log detailed reset components internally but provide simple response to API client
            log_info(&format!("Reset components: {:?}", reset_components));
            Ok(Json(json!({
                "success": true,
                "message": "Game reset",
                "game_id": app_state.game.id(),
                "created_at": app_state.game.created_at_string()
            })))
        }
        Err(errors) => {
            log_error_stderr(&format!("Game reset failed: {errors:?}"));
            Err(ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to reset game: {}", errors.join(", "))))
        }
    }
}

pub async fn handle_dumpgame(
    State(app_state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    log_info("Dump game request");

    // Check for client authentication header
    let client_id = match headers.get("X-Client-ID") {
        Some(header_value) => {
            match header_value.to_str() {
                Ok(id) => id,
                Err(_) => {
                    log_error("Invalid X-Client-ID header");
                    return Err(ApiError::new(StatusCode::BAD_REQUEST, "Invalid X-Client-ID header"));
                }
            }
        }
        None => {
            log_error("Missing X-Client-ID header");
            return Err(ApiError::new(StatusCode::UNAUTHORIZED, "Missing X-Client-ID header"));
        }
    };

    // Only allow board client (ID: "0000000000000000") to dump the game
    if client_id != BOARD_ID {
        log_error("Unauthorized: Only board client can dump the game");
        return Err(ApiError::new(StatusCode::FORBIDDEN, "Unauthorized: Only board client can dump the game"));
    }

    // Dump the game state to JSON
    match app_state.game.dump_to_json() {
        Ok(dump_message) => {
            log_info(&format!("Game manually dumped: {dump_message}"));
            Ok(Json(json!({
                "success": true,
                "message": dump_message,
                "game_id": app_state.game.id(),
                "game_ended": app_state.game.is_game_ended(),
                "bingo_reached": app_state.game.is_bingo_reached(),
                "pouch_empty": app_state.game.is_pouch_empty()
            })))
        }
        Err(dump_error) => {
            log_error(&format!("Manual game dump failed: {dump_error}"));
            Err(ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to dump game: {dump_error}")))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::Game;
    use crate::config::ServerConfig;
    use crate::server::AppState;
    use crate::client::RegisterRequest;
    use crate::card::GenerateCardsRequest;
    use axum::extract::{State, Query, Path};
    use axum::Json as JsonExtractor;
    use std::sync::Arc;
    use tokio;

    // Helper function to create test app state
    fn create_test_app_state() -> Arc<AppState> {
        let game = Game::new();
        let config = ServerConfig::default();
        Arc::new(AppState { game, config })
    }

    // Helper function to create a registered client
    async fn register_test_client(app_state: &Arc<AppState>, name: &str) -> String {
        let request = RegisterRequest {
            name: name.to_string(),
            client_type: "player".to_string(),
            nocard: Some(1),
        };

        let result = handle_register(State(app_state.clone()), JsonExtractor(request)).await;
        match result {
            Ok(response) => response.0.client_id,
            Err(_) => panic!("Failed to register test client"),
        }
    }

    #[tokio::test]
    async fn test_handle_register_new_client() {
        let app_state = create_test_app_state();
        let request = RegisterRequest {
            name: "test_player".to_string(),
            client_type: "player".to_string(),
            nocard: Some(2),
        };

        let result = handle_register(State(app_state.clone()), JsonExtractor(request)).await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.message, "Client 'test_player' registered successfully");
        assert!(!response.client_id.is_empty());
    }

    #[tokio::test]
    async fn test_handle_register_existing_client() {
        let app_state = create_test_app_state();
        let request = RegisterRequest {
            name: "existing_player".to_string(),
            client_type: "player".to_string(),
            nocard: Some(1),
        };

        // Register the client first time
        let first_result = handle_register(State(app_state.clone()), JsonExtractor(request.clone())).await;
        assert!(first_result.is_ok());
        let first_response = first_result.unwrap();

        // Try to register the same client again
        let second_result = handle_register(State(app_state.clone()), JsonExtractor(request)).await;
        assert!(second_result.is_ok());
        let second_response = second_result.unwrap();

        assert_eq!(first_response.client_id, second_response.client_id);
        assert_eq!(second_response.message, "Client 'existing_player' already registered");
    }

    #[tokio::test]
    async fn test_handle_register_after_game_started() {
        let app_state = create_test_app_state();

        // Start the game by extracting a number
        let _ = app_state.game.extract_number(0).unwrap();

        let request = RegisterRequest {
            name: "late_player".to_string(),
            client_type: "player".to_string(),
            nocard: Some(1),
        };

        let result = handle_register(State(app_state.clone()), JsonExtractor(request)).await;

        assert!(result.is_err());
        let error = result.unwrap_err();
        assert_eq!(error.status, StatusCode::CONFLICT);
        assert!(error.message.contains("Cannot register new clients after numbers have been extracted"));
    }

    #[tokio::test]
    async fn test_handle_client_info_existing() {
        let app_state = create_test_app_state();
        let client_id = register_test_client(&app_state, "info_test_player").await;

        let query = ClientNameQuery {
            name: Some("info_test_player".to_string()),
        };

        let result = handle_client_info(
            State(app_state.clone()),
            Query(query),
        ).await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.0.name, "info_test_player");
        assert_eq!(response.0.client_id, client_id);
        assert_eq!(response.0.client_type, "player");
    }

    #[tokio::test]
    async fn test_handle_client_info_nonexistent() {
        let app_state = create_test_app_state();

        let query = ClientNameQuery {
            name: Some("nonexistent_player".to_string()),
        };

        let result = handle_client_info(
            State(app_state.clone()),
            Query(query),
        ).await;

        assert!(result.is_err());
        let error = result.unwrap_err();
        assert_eq!(error.status, StatusCode::NOT_FOUND);
        assert!(error.message.contains("Client 'nonexistent_player' not found"));
    }

    #[tokio::test]
    async fn test_handle_client_info_by_id_existing() {
        let app_state = create_test_app_state();
        let client_id = register_test_client(&app_state, "id_test_player").await;

        let result = handle_client_info_by_id(
            State(app_state.clone()),
            Path(client_id.clone()),
        ).await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.name, "id_test_player");
        assert_eq!(response.client_id, client_id);
        assert_eq!(response.client_type, "player");
    }

    #[tokio::test]
    async fn test_handle_client_info_by_id_board_client() {
        let app_state = create_test_app_state();

        let result = handle_client_info_by_id(
            State(app_state.clone()),
            Path(BOARD_ID.to_string()),
        ).await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.name, "Board");
        assert_eq!(response.client_id, BOARD_ID);
        assert_eq!(response.client_type, "board");
    }

    #[tokio::test]
    async fn test_handle_client_info_by_id_nonexistent() {
        let app_state = create_test_app_state();

        let result = handle_client_info_by_id(
            State(app_state.clone()),
            Path("invalid_client_id".to_string()),
        ).await;

        assert!(result.is_err());
        let error = result.unwrap_err();
        assert_eq!(error.status, StatusCode::NOT_FOUND);
        assert!(error.message.contains("Client with ID 'invalid_client_id' not found"));
    }

    #[tokio::test]
    async fn test_handle_generate_cards_success() {
        let app_state = create_test_app_state();

        // Register a client with no cards during registration
        let register_request = RegisterRequest {
            name: "cards_test_player".to_string(),
            client_type: "player".to_string(),
            nocard: Some(0), // No cards during registration
        };

        let register_result = handle_register(State(app_state.clone()), JsonExtractor(register_request)).await;
        assert!(register_result.is_ok());
        let client_id = register_result.unwrap().0.client_id;

        let mut headers = HeaderMap::new();
        headers.insert("X-Client-ID", client_id.parse().unwrap());

        let request = GenerateCardsRequest { count: 3 };

        let result = handle_generate_cards(
            State(app_state.clone()),
            headers,
            JsonExtractor(request),
        ).await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.0.cards.len(), 3);
        assert_eq!(response.0.message, "Generated 3 cards successfully");
    }

    #[tokio::test]
    async fn test_handle_generate_cards_missing_client_id() {
        let app_state = create_test_app_state();
        let headers = HeaderMap::new(); // No X-Client-ID header

        let request = GenerateCardsRequest { count: 1 };

        let result = handle_generate_cards(
            State(app_state.clone()),
            headers,
            JsonExtractor(request),
        ).await;

        assert!(result.is_err());
        let error = result.unwrap_err();
        assert_eq!(error.status, StatusCode::BAD_REQUEST);
        assert!(error.message.contains("Client ID header (X-Client-ID) is required"));
    }

    #[tokio::test]
    async fn test_handle_generate_cards_unregistered_client() {
        let app_state = create_test_app_state();
        let mut headers = HeaderMap::new();
        headers.insert("X-Client-ID", "invalid_client_id".parse().unwrap());

        let request = GenerateCardsRequest { count: 1 };

        let result = handle_generate_cards(
            State(app_state.clone()),
            headers,
            JsonExtractor(request),
        ).await;

        assert!(result.is_err());
        let error = result.unwrap_err();
        assert_eq!(error.status, StatusCode::UNAUTHORIZED);
        assert!(error.message.contains("Client not registered"));
    }

    #[tokio::test]
    async fn test_handle_list_assigned_cards_success() {
        let app_state = create_test_app_state();
        let client_id = register_test_client(&app_state, "list_test_player").await;

        let mut headers = HeaderMap::new();
        headers.insert("X-Client-ID", client_id.parse().unwrap());

        let result = handle_list_assigned_cards(
            State(app_state.clone()),
            headers,
            Query(ClientIdQuery { client_id: None }),
        ).await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.0.cards.len(), 1); // Default nocard value from registration
        assert_eq!(response.0.cards[0].assigned_to, client_id);
    }

    #[tokio::test]
    async fn test_handle_list_assigned_cards_missing_client_id() {
        let app_state = create_test_app_state();
        let headers = HeaderMap::new(); // No X-Client-ID header

        let result = handle_list_assigned_cards(
            State(app_state.clone()),
            headers,
            Query(ClientIdQuery { client_id: None }),
        ).await;

        assert!(result.is_err());
        let error = result.unwrap_err();
        assert_eq!(error.status, StatusCode::BAD_REQUEST);
        assert!(error.message.contains("Client ID header (X-Client-ID) is required"));
    }

    #[tokio::test]
    async fn test_handle_get_assigned_card_success() {
        let app_state = create_test_app_state();
        let client_id = register_test_client(&app_state, "get_card_test_player").await;

        // Get the assigned card ID
        let mut headers = HeaderMap::new();
        headers.insert("X-Client-ID", client_id.parse().unwrap());

        let list_result = handle_list_assigned_cards(
            State(app_state.clone()),
            headers.clone(),
            Query(ClientIdQuery { client_id: None }),
        ).await;

        assert!(list_result.is_ok());
        let list_response = list_result.unwrap();
        assert!(!list_response.0.cards.is_empty());

        let card_id = &list_response.0.cards[0].card_id;

        let result = handle_get_assigned_card(
            State(app_state.clone()),
            Path(card_id.clone()),
            headers,
        ).await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.0["card_id"], *card_id);
    }

    #[tokio::test]
    async fn test_handle_get_assigned_card_not_found() {
        let app_state = create_test_app_state();
        let client_id = register_test_client(&app_state, "get_card_test_player").await;

        let mut headers = HeaderMap::new();
        headers.insert("X-Client-ID", client_id.parse().unwrap());

        let result = handle_get_assigned_card(
            State(app_state.clone()),
            Path("nonexistent_card_id".to_string()),
            headers,
        ).await;

        assert!(result.is_err());
        let error = result.unwrap_err();
        assert_eq!(error.status, StatusCode::NOT_FOUND);
        assert!(error.message.contains("Card not found"));
    }

    #[tokio::test]
    async fn test_handle_board_initial_state() {
        let app_state = create_test_app_state();

        let result = handle_board(
            State(app_state.clone()),
            Query(ClientIdQuery { client_id: None }),
        ).await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert!(response.0["numbers"].as_array().unwrap().is_empty());
        assert!(response.0["marked_numbers"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_handle_board_with_extracted_numbers() {
        let app_state = create_test_app_state();

        // Extract some numbers using the proper API handler
        let mut headers = HeaderMap::new();
        headers.insert("X-Client-ID", BOARD_ID.parse().unwrap()); // Board client

        let _ = handle_extract(
            State(app_state.clone()),
            headers.clone(),
            Query(ClientIdQuery { client_id: None }),
        ).await.unwrap();

        let _ = handle_extract(
            State(app_state.clone()),
            headers,
            Query(ClientIdQuery { client_id: None }),
        ).await.unwrap();

        let result = handle_board(
            State(app_state.clone()),
            Query(ClientIdQuery { client_id: None }),
        ).await;

        assert!(result.is_ok());
        let response = result.unwrap();

        // The board response is a direct serialization of the Board struct
        // which has 'numbers' and 'marked_numbers' fields
        assert_eq!(response.0["numbers"].as_array().unwrap().len(), 2);
        assert_eq!(response.0["marked_numbers"].as_array().unwrap().len(), 0); // marked_numbers is empty initially
    }

    #[tokio::test]
    async fn test_handle_pouch_initial_state() {
        let app_state = create_test_app_state();

        let result = handle_pouch(
            State(app_state.clone()),
            Query(ClientIdQuery { client_id: None }),
        ).await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.0["numbers"].as_array().unwrap().len(), 90); // Full pouch
    }

    #[tokio::test]
    async fn test_handle_pouch_after_extraction() {
        let app_state = create_test_app_state();

        // Extract a number
        let _ = app_state.game.extract_number(0).unwrap();

        let result = handle_pouch(
            State(app_state.clone()),
            Query(ClientIdQuery { client_id: None }),
        ).await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.0["numbers"].as_array().unwrap().len(), 89); // One less after extraction
    }

    #[tokio::test]
    async fn test_handle_scoremap_initial_state() {
        let app_state = create_test_app_state();

        let result = handle_scoremap(
            State(app_state.clone()),
            Query(ClientIdQuery { client_id: None }),
        ).await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.0["published_score"], 0);
        assert!(response.0["score_map"].as_object().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_handle_status() {
        let app_state = create_test_app_state();

        let result = handle_status(
            State(app_state.clone()),
            Query(ClientIdQuery { client_id: None }),
        ).await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.0["status"], "running");
        assert!(response.0["game_id"].is_string());
        assert!(response.0["created_at"].is_string());
        assert_eq!(response.0["numbers_extracted"], 0);
        assert_eq!(response.0["scorecard"], 0);
        assert_eq!(response.0["server"], "axum");
    }

    #[tokio::test]
    async fn test_handle_running_game_id() {
        let app_state = create_test_app_state();

        let result = handle_running_game_id(State(app_state.clone())).await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert!(response["game_id"].is_string());
        assert!(response["created_at"].is_string());
        assert!(response["created_at_timestamp"].is_object());
    }

    #[tokio::test]
    async fn test_handle_extract_success() {
        let app_state = create_test_app_state();
        let mut headers = HeaderMap::new();
        headers.insert("X-Client-ID", BOARD_ID.parse().unwrap()); // Board client

        let result = handle_extract(
            State(app_state.clone()),
            headers,
            Query(ClientIdQuery { client_id: None }),
        ).await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.0["success"], true);
        assert!(response.0["extracted_number"].is_number());
        assert_eq!(response.0["numbers_remaining"], 89);
        assert_eq!(response.0["total_extracted"], 1);
    }

    #[tokio::test]
    async fn test_handle_extract_unauthorized() {
        let app_state = create_test_app_state();
        let client_id = register_test_client(&app_state, "extract_test_player").await;

        let mut headers = HeaderMap::new();
        headers.insert("X-Client-ID", client_id.parse().unwrap()); // Regular client, not board

        let result = handle_extract(
            State(app_state.clone()),
            headers,
            Query(ClientIdQuery { client_id: None }),
        ).await;

        assert!(result.is_err());
        let error = result.unwrap_err();
        assert_eq!(error.status, StatusCode::FORBIDDEN);
        assert!(error.message.contains("Unauthorized: Only board client can extract numbers"));
    }

    #[tokio::test]
    async fn test_handle_extract_missing_client_id() {
        let app_state = create_test_app_state();
        let headers = HeaderMap::new(); // No X-Client-ID header

        let result = handle_extract(
            State(app_state.clone()),
            headers,
            Query(ClientIdQuery { client_id: None }),
        ).await;

        assert!(result.is_err());
        let error = result.unwrap_err();
        assert_eq!(error.status, StatusCode::BAD_REQUEST);
        assert!(error.message.contains("Client ID header (X-Client-ID) is required"));
    }

    #[tokio::test]
    async fn test_handle_newgame_success() {
        let app_state = create_test_app_state();
        let mut headers = HeaderMap::new();
        headers.insert("X-Client-ID", BOARD_ID.parse().unwrap()); // Board client

        // Register a client and extract some numbers to have game state
        let _ = register_test_client(&app_state, "newgame_test_player").await;
        let _ = app_state.game.extract_number(0).unwrap();

        let result = handle_newgame(State(app_state.clone()), headers).await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response["success"], true);
        assert_eq!(response["message"], "New game started successfully");
        assert!(response["game_id"].is_string());
        assert!(response["created_at"].is_string());
        assert!(response["reset_components"].is_array());
    }

    #[tokio::test]
    async fn test_handle_newgame_unauthorized() {
        let app_state = create_test_app_state();
        let client_id = register_test_client(&app_state, "newgame_test_player").await;

        let mut headers = HeaderMap::new();
        headers.insert("X-Client-ID", client_id.parse().unwrap()); // Regular client, not board

        let result = handle_newgame(State(app_state.clone()), headers).await;

        assert!(result.is_err());
        let error = result.unwrap_err();
        assert_eq!(error.status, StatusCode::FORBIDDEN);
        assert!(error.message.contains("Unauthorized: Only board client can reset the game"));
    }

    #[tokio::test]
    async fn test_handle_dumpgame_success() {
        let app_state = create_test_app_state();
        let mut headers = HeaderMap::new();
        headers.insert("X-Client-ID", BOARD_ID.parse().unwrap()); // Board client

        // Create some game state
        let _ = register_test_client(&app_state, "dumpgame_test_player").await;
        let _ = app_state.game.extract_number(0).unwrap();

        let result = handle_dumpgame(State(app_state.clone()), headers).await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.0["success"], true);
        assert!(response.0["message"].is_string());
        assert!(response.0["game_id"].is_string());
        assert!(response.0["game_ended"].is_boolean());
        assert!(response.0["bingo_reached"].is_boolean());
        assert!(response.0["pouch_empty"].is_boolean());
    }

    #[tokio::test]
    async fn test_handle_dumpgame_unauthorized() {
        let app_state = create_test_app_state();
        let client_id = register_test_client(&app_state, "dumpgame_test_player").await;

        let mut headers = HeaderMap::new();
        headers.insert("X-Client-ID", client_id.parse().unwrap()); // Regular client, not board

        let result = handle_dumpgame(State(app_state.clone()), headers).await;

        assert!(result.is_err());
        let error = result.unwrap_err();
        assert_eq!(error.status, StatusCode::FORBIDDEN);
        assert!(error.message.contains("Unauthorized: Only board client can dump the game"));
    }

    #[tokio::test]
    async fn test_handle_dumpgame_missing_client_id() {
        let app_state = create_test_app_state();
        let headers = HeaderMap::new(); // No X-Client-ID header

        let result = handle_dumpgame(State(app_state.clone()), headers).await;

        assert!(result.is_err());
        let error = result.unwrap_err();
        assert_eq!(error.status, StatusCode::UNAUTHORIZED);
        assert!(error.message.contains("Missing X-Client-ID header"));
    }

    #[tokio::test]
    async fn test_api_error_into_response() {
        let error = ApiError::new(StatusCode::NOT_FOUND, "Test error message");
        let response = error.into_response();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_client_flow_integration() {
        let app_state = create_test_app_state();

        // Register a client
        let client_id = register_test_client(&app_state, "integration_test_player").await;

        // List assigned cards
        let mut headers = HeaderMap::new();
        headers.insert("X-Client-ID", client_id.parse().unwrap());

        let cards_result = handle_list_assigned_cards(
            State(app_state.clone()),
            headers.clone(),
            Query(ClientIdQuery { client_id: None }),
        ).await;
        assert!(cards_result.is_ok());
        let cards = cards_result.unwrap();
        assert_eq!(cards.0.cards.len(), 1);

        // Get specific card
        let card_id = &cards.0.cards[0].card_id;
        let card_result = handle_get_assigned_card(
            State(app_state.clone()),
            Path(card_id.clone()),
            headers.clone(),
        ).await;
        assert!(card_result.is_ok());

        // Extract a number (as board client)
        let mut board_headers = HeaderMap::new();
        board_headers.insert("X-Client-ID", BOARD_ID.parse().unwrap());

        let extract_result = handle_extract(
            State(app_state.clone()),
            board_headers,
            Query(ClientIdQuery { client_id: None }),
        ).await;
        assert!(extract_result.is_ok());

        // Check board state
        let board_result = handle_board(
            State(app_state.clone()),
            Query(ClientIdQuery { client_id: None }),
        ).await;
        assert!(board_result.is_ok());
        let board = board_result.unwrap();
        assert_eq!(board.0["numbers"].as_array().unwrap().len(), 1);

        // Check status
        let status_result = handle_status(
            State(app_state.clone()),
            Query(ClientIdQuery { client_id: None }),
        ).await;
        assert!(status_result.is_ok());
        let status = status_result.unwrap();
        assert_eq!(status.0["numbers_extracted"], 1);
    }
}
