use std::sync::Arc;

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
use crate::logging::{log, LogLevel};
use crate::server::AppState;
use crate::game::Game;

const MODULE_NAME: &str = "api_handlers";

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

// Helper function to get a game from the registry by ID
async fn get_game_from_registry(app_state: &Arc<AppState>, game_id: &str) -> Result<Arc<Game>, ApiError> {
    match app_state.game_registry.get_game(game_id) {
        Ok(Some(game)) => Ok(game),
        Ok(None) => Err(ApiError::new(StatusCode::NOT_FOUND, format!("Game with ID '{game_id}' not found"))),
        Err(e) => Err(ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to access game registry: {e}"))),
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

pub async fn handle_join(
    Path(game_id): Path<String>,
    State(app_state): State<Arc<AppState>>,
    JsonExtractor(request): JsonExtractor<RegisterRequest>,
) -> Result<Json<RegisterResponse>, ApiError> {
    log(LogLevel::Info, MODULE_NAME, &format!("Client registration request for game '{game_id}': {request:?}"));

    let game = get_game_from_registry(&app_state, &game_id).await?;

    let client_name = &request.name;
    let client_type = &request.client_type;

    // First, check if the client already exists globally
    let client_info = match app_state.global_client_registry.get_by_name(client_name) {
        Ok(Some(existing_client)) => {
            // Client exists globally, reuse their info
            existing_client
        }
        Ok(None) => {
            // Client doesn't exist globally, create new one
            let new_client = ClientInfo::new(client_name, client_type, "");  // Empty email for now

            // Add to global registry
            if let Err(e) = app_state.global_client_registry.insert(new_client.clone()) {
                log(LogLevel::Error, MODULE_NAME, &format!("Failed to add client to global registry: {e}"));
                return Err(ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "Failed to register client globally"));
            }

            new_client
        }
        Err(e) => {
            log(LogLevel::Error, MODULE_NAME, &format!("Failed to access global client registry: {e}"));
            return Err(ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "Failed to access global client registry"));
        }
    };

    let client_id = client_info.id.clone();

    // Log with client ID now that we have it
    log(LogLevel::Info, MODULE_NAME, &format!("[Client: {client_id}] Processing registration for game '{game_id}'"));

    // Check if client is already registered to this specific game
    if game.contains_client(&client_id) {
        return Ok(Json(RegisterResponse {
            client_id: client_id.clone(),
            message: format!("Client '{client_name}' already registered in game '{game_id}'"),
        }));
    }

    // Try to register the client to this specific game (will fail if numbers have been extracted)
    match game.add_client(client_id.clone()) {
        Ok(added) => {
            if added {
                log(LogLevel::Info, MODULE_NAME, &format!("[Client: {client_id}] Registered successfully in game '{game_id}'"));
            } else {
                log(LogLevel::Info, MODULE_NAME, &format!("[Client: {client_id}] Already registered in game '{game_id}'"));
            }
        }
        Err(e) => {
            log(LogLevel::Error, MODULE_NAME, &format!("[Client: {client_id}] Failed to register in game '{game_id}': {e}"));
            return Err(ApiError::new(StatusCode::CONFLICT, e));
        }
    }

    // Set the game-specific client type for this client in this game
    if let Err(e) = game.set_client_type(&client_id, client_type) {
        log(LogLevel::Error, MODULE_NAME, &format!("[Client: {client_id}] Failed to set client type in game '{game_id}': {e}"));
        return Err(ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "Failed to set client type for game"));
    }

    // Check if client requested cards during registration, default to 1 if not specified
    let card_count = request.nocard.unwrap_or(1);
    log(LogLevel::Info, MODULE_NAME, &format!("[Client: {client_id}] Generating {card_count} cards during registration"));

    // Generate the requested number of cards using the card manager
    if let Ok(mut manager) = game.card_manager().lock() {
        manager.assign_cards_with_type(&client_id, card_count, Some(client_type));
        log(LogLevel::Info, MODULE_NAME, &format!("[Client: {client_id}] Generated and assigned {card_count} cards in game '{game_id}'"));
    } else {
        log(LogLevel::Warning, MODULE_NAME, &format!("[Client: {client_id}] Failed to acquire card manager lock in game '{game_id}'"));
    }

    Ok(Json(RegisterResponse {
        client_id,
        message: format!("Client '{client_name}' registered successfully in game '{game_id}'"),
    }))
}

pub async fn handle_global_clientinfo(
    State(app_state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(params): Query<ClientNameQuery>,
) -> Result<Json<ClientInfoResponse>, ApiError> {
    let client_name = params.name.unwrap_or_default();

    // Get optional client ID from headers for logging
    let client_id_opt = if let Some(header_value) = headers.get("X-Client-ID") {
        if let Ok(id) = header_value.to_str() {
            Some(id.to_string())
        } else {
            None
        }
    } else {
        None
    };

    // Log with client ID if available
    if let Some(client_id) = &client_id_opt {
        log(LogLevel::Info, MODULE_NAME, &format!("[Client: {client_id}] Client info request for: {client_name}"));
    } else {
        log(LogLevel::Info, MODULE_NAME, &format!("Client info request for: {client_name}"));
    }

    match app_state.global_client_registry.get_by_name(&client_name) {
        Ok(Some(client)) => {
            Ok(Json(ClientInfoResponse {
                client_id: client.id.clone(),
                name: client.name.clone(),
                client_type: client.client_type.clone(),
                registered_at: format!("{:?}", client.registered_at),
            }))
        }
        Ok(None) => {
            Err(ApiError::new(StatusCode::NOT_FOUND, format!("Client '{client_name}' not found")))
        }
        Err(e) => {
            log(LogLevel::Error, MODULE_NAME, &format!("Failed to access global client registry: {e}"));
            Err(ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "Failed to access global client registry"))
        }
    }
}

pub async fn handle_global_clientinfo_by_id(
    State(app_state): State<Arc<AppState>>,
    Path(client_id): Path<String>,
) -> Result<Json<ClientInfoResponse>, ApiError> {
    log(LogLevel::Info, MODULE_NAME, &format!("[Client: {client_id}] Client info by ID request"));

    // Handle special case for board client ID
    if client_id == BOARD_ID {
        return Ok(Json(ClientInfoResponse {
            client_id: client_id.clone(),
            name: "Board".to_string(),
            client_type: "board".to_string(),
            registered_at: "System".to_string(),
        }));
    }

    // Search in global client registry by client ID
    match app_state.global_client_registry.get_all_clients() {
        Ok(clients) => {
            for client_info in clients {
                if client_info.id == client_id {
                    return Ok(Json(ClientInfoResponse {
                        client_id: client_info.id.clone(),
                        name: client_info.name.clone(),
                        client_type: client_info.client_type.clone(),
                        registered_at: format!("{:?}", client_info.registered_at),
                    }));
                }
            }
        }
        Err(e) => {
            log(LogLevel::Error, MODULE_NAME, &format!("[Client: {client_id}] Failed to access global client registry: {e}"));
            return Err(ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "Failed to access global client registry"));
        }
    }

    // Client not found
    Err(ApiError::new(StatusCode::NOT_FOUND, format!("Client with ID '{client_id}' not found")))
}

pub async fn handle_global_register(
    State(app_state): State<Arc<AppState>>,
    headers: HeaderMap,
    JsonExtractor(request): JsonExtractor<RegisterRequest>,
) -> Result<Json<RegisterResponse>, ApiError> {
    // Get optional client ID from headers for logging
    let client_id_opt = if let Some(header_value) = headers.get("X-Client-ID") {
        if let Ok(id) = header_value.to_str() {
            Some(id.to_string())
        } else {
            None
        }
    } else {
        None
    };

    // Log with client ID if available
    if let Some(client_id) = &client_id_opt {
        log(LogLevel::Info, MODULE_NAME, &format!("[Client: {client_id}] Global client registration request: {request:?}"));
    } else {
        log(LogLevel::Info, MODULE_NAME, &format!("Global client registration request: {request:?}"));
    }

    let client_name = &request.name;
    let client_type = &request.client_type;
    let email = request.email.as_deref().unwrap_or("");  // Use provided email or empty string

    // Check if the client already exists globally
    match app_state.global_client_registry.get_by_name(client_name) {
        Ok(Some(existing_client)) => {
            // Client already exists globally
            log(LogLevel::Info, MODULE_NAME, &format!("[Client: {}] Client '{}' already registered globally", existing_client.id, client_name));
            Ok(Json(RegisterResponse {
                client_id: existing_client.id.clone(),
                message: format!("Client '{client_name}' already registered globally"),
            }))
        }
        Ok(None) => {
            // Client doesn't exist globally, create new one
            let new_client = ClientInfo::new(client_name, client_type, email);

            // Add to global registry
            match app_state.global_client_registry.insert(new_client.clone()) {
                Ok(_) => {
                    log(LogLevel::Info, MODULE_NAME, &format!("[Client: {}] Successfully registered client '{}' globally with ID: {}", new_client.id, client_name, new_client.id));
                    Ok(Json(RegisterResponse {
                        client_id: new_client.id.clone(),
                        message: format!("Client '{client_name}' registered successfully globally"),
                    }))
                }
                Err(e) => {
                    log(LogLevel::Error, MODULE_NAME, &format!("Failed to add client to global registry: {e}"));
                    Err(ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "Failed to register client globally"))
                }
            }
        }
        Err(e) => {
            log(LogLevel::Error, MODULE_NAME, &format!("Failed to access global client registry: {e}"));
            Err(ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "Failed to access global client registry"))
        }
    }
}

pub async fn handle_generatecards(
    State(app_state): State<Arc<AppState>>,
    Path(game_id): Path<String>,
    headers: HeaderMap,
    JsonExtractor(request): JsonExtractor<GenerateCardsRequest>,
) -> Result<Json<GenerateCardsResponse>, ApiError> {
    // Get client ID from headers first, so we can use it in logging
    let client_id = if let Some(header_value) = headers.get("X-Client-ID") {
        if let Ok(id) = header_value.to_str() {
            id.to_string()
        } else {
            log(LogLevel::Error, MODULE_NAME, "Invalid client ID in header");
            return Err(ApiError::new(StatusCode::BAD_REQUEST, "Invalid client ID in header"));
        }
    } else {
        log(LogLevel::Error, MODULE_NAME, "Client ID header (X-Client-ID) is required");
        return Err(ApiError::new(StatusCode::BAD_REQUEST, "Client ID header (X-Client-ID) is required"));
    };

    log(LogLevel::Info, MODULE_NAME, &format!("[Client: {client_id}] Generate cards request for game: {game_id}"));

    let game = get_game_from_registry(&app_state, &game_id).await?;

    // Verify client is registered and get their info
    match game.get_client_info(&client_id, &app_state.global_client_registry) {
        Ok(Some(_client_info)) => {
            // Client is registered and found in global registry
        }
        Ok(None) => {
            log(LogLevel::Error, MODULE_NAME, &format!("[Client: {client_id}] Not registered"));
            return Err(ApiError::new(StatusCode::UNAUTHORIZED, "Client not registered"));
        }
        Err(e) => {
            log(LogLevel::Error, MODULE_NAME, &format!("[Client: {client_id}] Failed to verify registration: {e}"));
            return Err(ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "Failed to verify client registration"));
        }
    }

    // Check if client already has cards assigned (prevent duplicate generation)
    if let Ok(manager) = game.card_manager().lock() {
        if let Some(existing_cards) = manager.get_client_cards(&client_id) {
            if !existing_cards.is_empty() {
                log(LogLevel::Error, MODULE_NAME, &format!("[Client: {client_id}] Already has cards assigned. Card generation is only allowed during registration."));
                return Err(ApiError::new(StatusCode::CONFLICT, "Client already has cards assigned. Card generation is only allowed during registration."));
            }
        }
    }

    // Get client type for proper card assignment
    let client_type = match app_state.global_client_registry.get_by_client_id(&client_id) {
        Ok(Some(info)) => Some(info.client_type.clone()),
        _ => None,
    };

    // Generate cards using the CardAssignmentManager
    let card_infos = if let Ok(mut manager) = game.card_manager().lock() {
        let (cards, _) = manager.assign_cards_with_type(&client_id, request.count, client_type.as_deref());
        cards
    } else {
        log(LogLevel::Error, MODULE_NAME, "Failed to acquire card manager lock");
        return Err(ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "Failed to acquire card manager lock"));
    };

    log(LogLevel::Info, MODULE_NAME, &format!("[Client: {}] Generated {} cards for client {}", client_id, card_infos.len(), client_id));

    // Create response
    let response = GenerateCardsResponse {
        cards: card_infos,
        message: format!("Generated {} cards successfully", request.count),
    };

    Ok(Json(response))
}

pub async fn handle_listassignedcards(
    State(app_state): State<Arc<AppState>>,
    Path(game_id): Path<String>,
    headers: HeaderMap,
    Query(_params): Query<ClientIdQuery>,
) -> Result<Json<ListAssignedCardsResponse>, ApiError> {
    // Get client ID from headers first
    let client_id = if let Some(header_value) = headers.get("X-Client-ID") {
        if let Ok(id) = header_value.to_str() {
            id.to_string()
        } else {
            log(LogLevel::Error, MODULE_NAME, "Invalid client ID in header");
            return Err(ApiError::new(StatusCode::BAD_REQUEST, "Invalid client ID in header"));
        }
    } else {
        log(LogLevel::Error, MODULE_NAME, "Client ID header (X-Client-ID) is required");
        return Err(ApiError::new(StatusCode::BAD_REQUEST, "Client ID header (X-Client-ID) is required"));
    };

    log(LogLevel::Info, MODULE_NAME, &format!("[Client: {client_id}] List assigned cards request for game: {game_id}"));

    let game = get_game_from_registry(&app_state, &game_id).await?;

    // Verify client is registered and get their info
    match game.get_client_info(&client_id, &app_state.global_client_registry) {
        Ok(Some(_client_info)) => {
            // Client is registered and found in global registry
        }
        Ok(None) => {
            log(LogLevel::Error, MODULE_NAME, &format!("[Client: {client_id}] Client not registered"));
            return Err(ApiError::new(StatusCode::UNAUTHORIZED, "Client not registered"));
        }
        Err(e) => {
            log(LogLevel::Error, MODULE_NAME, &format!("[Client: {client_id}] Failed to verify client registration: {e}"));
            return Err(ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "Failed to verify client registration"));
        }
    }

    // Get client's assigned cards
    let assigned_cards = if let Ok(manager) = game.card_manager().lock() {
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

pub async fn handle_getassignedcard(
    State(app_state): State<Arc<AppState>>,
    Path((game_id, card_id)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    // Get client ID from headers first
    let client_id = if let Some(header_value) = headers.get("X-Client-ID") {
        if let Ok(id) = header_value.to_str() { id.to_string() } else {
            log(LogLevel::Error, MODULE_NAME, "Invalid client ID in header");
            return Err(ApiError::new(StatusCode::BAD_REQUEST, "Invalid client ID in header"));
        }
    } else {
        log(LogLevel::Error, MODULE_NAME, "Client ID header (X-Client-ID) is required");
        return Err(ApiError::new(StatusCode::BAD_REQUEST, "Client ID header (X-Client-ID) is required"));
    };

    log(LogLevel::Info, MODULE_NAME, &format!("[Client: {client_id}] Get assigned card request for game: {game_id}, card ID: {card_id}"));

    let game = get_game_from_registry(&app_state, &game_id).await?;

    // Verify client is registered and get their info
    let _client_info = match game.get_client_info(&client_id, &app_state.global_client_registry) {
        Ok(Some(info)) => info,
        Ok(None) => {
            log(LogLevel::Error, MODULE_NAME, &format!("[Client: {client_id}] Client not registered: {client_id}"));
            return Err(ApiError::new(StatusCode::UNAUTHORIZED, "Client not registered"));
        }
        Err(e) => {
            log(LogLevel::Error, MODULE_NAME, &format!("[Client: {client_id}] Failed to check client registration: {e}"));
            return Err(ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "Internal error"));
        }
    };

    // Get the card assignment
    let card_assignment = if let Ok(manager) = game.card_manager().lock() {
        manager.get_card_assignment(&card_id).cloned()
    } else {
        None
    };

    // Verify the card exists and belongs to the client
    let card_assignment = if let Some(assignment) = card_assignment {
        if assignment.client_id != client_id {
            log(LogLevel::Error, MODULE_NAME, &format!("Card {card_id} not assigned to client {client_id}"));
            return Err(ApiError::new(StatusCode::FORBIDDEN, "Card not assigned to this client"));
        }
        assignment
    } else {
        log(LogLevel::Error, MODULE_NAME, &format!("Card not found: {card_id}"));
        return Err(ApiError::new(StatusCode::NOT_FOUND, "Card not found"));
    };

    // Create response
    let card_info = crate::card::CardInfo {
        card_id: card_assignment.card_id,
        card_data: card_assignment.card_data.clone(),
    };

    Ok(Json(serde_json::to_value(&card_info).unwrap()))
}

pub async fn handle_board(
    State(app_state): State<Arc<AppState>>,
    Path(game_id): Path<String>,
    headers: HeaderMap,
    Query(_params): Query<ClientIdQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    // Get optional client ID from headers for logging
    let client_id_opt = if let Some(header_value) = headers.get("X-Client-ID") {
        if let Ok(id) = header_value.to_str() {
            Some(id.to_string())
        } else {
            None
        }
    } else {
        None
    };

    // Log with client ID if available
    if let Some(client_id) = &client_id_opt {
        log(LogLevel::Info, MODULE_NAME, &format!("[Client: {client_id}] Board request for game: {game_id}"));
    } else {
        log(LogLevel::Info, MODULE_NAME, &format!("Board request for game: {game_id}"));
    }

    let game = get_game_from_registry(&app_state, &game_id).await?;

    let board_data = if let Ok(board) = game.board().lock() {
        serde_json::to_value(&*board).unwrap_or_else(|_| serde_json::json!({}))
    } else {
        serde_json::to_value(Board::new()).unwrap_or_else(|_| serde_json::json!({}))
    };

    Ok(Json(board_data))
}

pub async fn handle_pouch(
    State(app_state): State<Arc<AppState>>,
    Path(game_id): Path<String>,
    headers: HeaderMap,
    Query(_params): Query<ClientIdQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    // Check for optional client ID in headers
    let client_id_opt = if let Some(header_value) = headers.get("X-Client-ID") {
        if let Ok(id) = header_value.to_str() {
            Some(id.to_string())
        } else {
            None
        }
    } else {
        None
    };

    // Log with client ID if available
    if let Some(client_id) = &client_id_opt {
        log(LogLevel::Info, MODULE_NAME, &format!("[Client: {client_id}] Pouch request for game: {game_id}"));
    } else {
        log(LogLevel::Info, MODULE_NAME, &format!("Pouch request for game: {game_id}"));
    }

    let game = get_game_from_registry(&app_state, &game_id).await?;

    let pouch_data = if let Ok(pouch) = game.pouch().lock() {
        serde_json::to_value(&*pouch).unwrap_or_else(|_| serde_json::json!({}))
    } else {
        serde_json::to_value(Pouch::new()).unwrap_or_else(|_| serde_json::json!({}))
    };

    Ok(Json(pouch_data))
}

pub async fn handle_scoremap(
    State(app_state): State<Arc<AppState>>,
    Path(game_id): Path<String>,
    headers: HeaderMap,
    Query(_params): Query<ClientIdQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    // Get optional client ID from headers for logging
    let client_id_opt = if let Some(header_value) = headers.get("X-Client-ID") {
        if let Ok(id) = header_value.to_str() {
            Some(id.to_string())
        } else {
            None
        }
    } else {
        None
    };

    // Log with client ID if available
    if let Some(client_id) = &client_id_opt {
        log(LogLevel::Info, MODULE_NAME, &format!("[Client: {client_id}] Score map request for game: {game_id}"));
    } else {
        log(LogLevel::Info, MODULE_NAME, &format!("Score map request for game: {game_id}"));
    }

    let game = get_game_from_registry(&app_state, &game_id).await?;

    let scorecard_data = if let Ok(scorecard) = game.scorecard().lock() {
        serde_json::to_value(&*scorecard).unwrap_or_else(|_| serde_json::json!({}))
    } else {
        serde_json::to_value(ScoreCard::new()).unwrap_or_else(|_| serde_json::json!({}))
    };

    Ok(Json(scorecard_data))
}

pub async fn handle_status(
    State(app_state): State<Arc<AppState>>,
    Path(game_id): Path<String>,
    headers: HeaderMap,
    Query(_params): Query<ClientIdQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    // Get optional client ID from headers for logging
    let client_id_opt = if let Some(header_value) = headers.get("X-Client-ID") {
        if let Ok(id) = header_value.to_str() {
            Some(id.to_string())
        } else {
            None
        }
    } else {
        None
    };

    // Log with client ID if available
    if let Some(client_id) = &client_id_opt {
        log(LogLevel::Info, MODULE_NAME, &format!("[Client: {client_id}] Status request for game: {game_id}"));
    } else {
        log(LogLevel::Info, MODULE_NAME, &format!("Status request for game: {game_id}"));
    }

    let game = get_game_from_registry(&app_state, &game_id).await?;

    let status = game.status();
    let board_len = game.board_length();
    let scorecard = game.published_score();
    let player_count = game.player_count();
    let card_count = game.card_count();

    let mut response = json!({
        "status": status.as_str().to_lowercase(),
        "game_id": game.id(),
        "created_at": game.created_at_string(),
        "players": player_count.to_string(),
        "cards": card_count.to_string(),
        "numbers_extracted": board_len,
        "scorecard": scorecard,
    });

    // Add closed_at only if the game is closed
    if status == crate::game::GameStatus::Closed {
        // For now, use current time as placeholder - in production this should be tracked properly
        let closed_time = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string();
        response.as_object_mut().unwrap().insert("closed_at".to_string(), serde_json::Value::String(closed_time));
    }

    Ok(Json(response))
}

pub async fn handle_extract(
    State(app_state): State<Arc<AppState>>,
    Path(game_id): Path<String>,
    headers: HeaderMap,
    Query(_params): Query<ClientIdQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    // Get client ID from headers for authentication first
    let client_id = if let Some(header_value) = headers.get("X-Client-ID") {
        if let Ok(id) = header_value.to_str() { id.to_string() } else {
            log(LogLevel::Error, MODULE_NAME, "Invalid client ID in header");
            return Err(ApiError::new(StatusCode::BAD_REQUEST, "Invalid client ID in header"));
        }
    } else {
        log(LogLevel::Error, MODULE_NAME, "Client ID header (X-Client-ID) is required");
        return Err(ApiError::new(StatusCode::BAD_REQUEST, "Client ID header (X-Client-ID) is required"));
    };

    log(LogLevel::Info, MODULE_NAME, &format!("[Client: {client_id}] Extract request for game: {game_id}"));

    let game = get_game_from_registry(&app_state, &game_id).await?;

    // Check if the client is registered to this game
    if !game.contains_client(&client_id) {
        log(LogLevel::Error, MODULE_NAME, &format!("Client {client_id} is not registered to game {game_id}"));
        return Err(ApiError::new(StatusCode::FORBIDDEN, "Client must be registered to this game"));
    }

    // Only allow board client type to extract numbers - check game-specific client type
    match game.is_client_type(&client_id, "board") {
        Ok(is_board) => {
            if !is_board {
                log(LogLevel::Error, MODULE_NAME, &format!("Unauthorized: Only board clients can extract numbers, client ID: {client_id}"));
                return Err(ApiError::new(StatusCode::FORBIDDEN, "Unauthorized: Only board clients can extract numbers"));
            }
        }
        Err(e) => {
            log(LogLevel::Error, MODULE_NAME, &format!("Failed to check client type for '{client_id}' in game '{game_id}': {e}"));
            return Err(ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "Failed to verify client authorization"));
        }
    }

    // Check if BINGO has been reached - if so, no more extractions allowed
    if game.is_bingo_reached() {
        return Err(ApiError::new(StatusCode::CONFLICT, "Game over: BINGO has been reached. No more numbers can be extracted."));
    }

    // Extract a number using the game's coordinated extraction logic
    match game.extract_number(0, Some(&client_id)) {
        Ok((extracted_number, _new_working_score)) => {
            // Get current pouch and board state for response using Game methods
            let numbers_remaining = game.pouch_length();
            let total_extracted = game.board_length();

            // Check if BINGO was reached after this extraction and dump game state if so
            if game.is_bingo_reached() {
                match game.dump_to_json() {
                    Ok(dump_message) => {
                        log(LogLevel::Info, MODULE_NAME, &format!("Game ended with BINGO! {dump_message}"));
                    }
                    Err(dump_error) => {
                        log(LogLevel::Error, MODULE_NAME, &format!("Failed to dump game state: {dump_error}"));
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
            log(LogLevel::Error, MODULE_NAME, &format!("Failed to extract number: {error_msg}"));
            if error_msg.contains("empty") {
                Err(ApiError::new(StatusCode::CONFLICT, error_msg))
            } else {
                Err(ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, error_msg))
            }
        }
    }
}

pub async fn handle_global_newgame(
    State(app_state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    // Get client ID from headers for authentication first
    let client_id = if let Some(header_value) = headers.get("X-Client-ID") {
        if let Ok(id) = header_value.to_str() { id.to_string() } else {
            log(LogLevel::Error, MODULE_NAME, "Invalid client ID in header");
            return Err(ApiError::new(StatusCode::BAD_REQUEST, "Invalid client ID in header"));
        }
    } else {
        log(LogLevel::Error, MODULE_NAME, "Client ID header (X-Client-ID) is required");
        return Err(ApiError::new(StatusCode::BAD_REQUEST, "Client ID header (X-Client-ID) is required"));
    };

    log(LogLevel::Info, MODULE_NAME, &format!("[Client: {client_id}] New game request"));

    // Only allow board client (ID: "0000000000000000") to create a new game
    if client_id != BOARD_ID {
        log(LogLevel::Error, MODULE_NAME, &format!("[Client: {client_id}] Unauthorized: Only board client can create a new game"));
        return Err(ApiError::new(StatusCode::FORBIDDEN, "Unauthorized: Only board client can create a new game"));
    }

    // Create a completely new game
    let new_game = Game::new();
    let new_game_id = new_game.id();
    let new_game_created_at = new_game.created_at_string();

    log(LogLevel::Info, MODULE_NAME, &format!("Created new game: {}", new_game.game_info()));

    // Add the new game to the registry
    let new_game_arc = Arc::new(new_game.clone());
    match app_state.game_registry.add_game(new_game_arc) {
        Ok(registered_id) => {
            log(LogLevel::Info, MODULE_NAME, &format!("Registered new game in registry: {registered_id}"));
        }
        Err(e) => {
            log(LogLevel::Error, MODULE_NAME, &format!("Failed to register new game in registry: {e}"));
            return Err(ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to register new game: {e}")));
        }
    }

    // Note: The new game is created and registered in the registry, but the current AppState.game
    // still points to the old game. In a future implementation, we could enhance this to
    // switch the active game, but for now this creates a new game that can be accessed via
    // the /gameslist endpoint and potentially switched to in the future.

    Ok(Json(json!({
        "success": true,
        "message": "New game created successfully",
        "game_id": new_game_id,
        "created_at": new_game_created_at,
        "note": "New game created and registered. Access it via /gameslist endpoint."
    })))
}

pub async fn handle_dumpgame(
    State(app_state): State<Arc<AppState>>,
    Path(game_id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    // Check for client authentication header first
    let client_id = if let Some(header_value) = headers.get("X-Client-ID") {
        if let Ok(id) = header_value.to_str() { id } else {
            log(LogLevel::Error, MODULE_NAME, "Invalid X-Client-ID header");
            return Err(ApiError::new(StatusCode::BAD_REQUEST, "Invalid X-Client-ID header"));
        }
    } else {
        log(LogLevel::Error, MODULE_NAME, "Missing X-Client-ID header");
        return Err(ApiError::new(StatusCode::UNAUTHORIZED, "Missing X-Client-ID header"));
    };

    log(LogLevel::Info, MODULE_NAME, &format!("[Client: {client_id}] Dump game request for game: {game_id}"));

    let game = get_game_from_registry(&app_state, &game_id).await?;

    // Only allow board client type to dump the game - check game-specific client type
    match game.is_client_type(client_id, "board") {
        Ok(is_board) => {
            if !is_board {
                log(LogLevel::Error, MODULE_NAME, &format!("Unauthorized: Only board clients can dump the game, client ID: {client_id}"));
                return Err(ApiError::new(StatusCode::FORBIDDEN, "Unauthorized: Only board clients can dump the game"));
            }
        }
        Err(e) => {
            log(LogLevel::Error, MODULE_NAME, &format!("Failed to check client type for '{client_id}' in game '{game_id}': {e}"));
            return Err(ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "Failed to verify client authorization"));
        }
    }

    // Dump the game state to JSON
    match game.dump_to_json() {
        Ok(dump_message) => {
            log(LogLevel::Info, MODULE_NAME, &format!("Game manually dumped: {dump_message}"));
            Ok(Json(json!({
                "success": true,
                "message": dump_message,
                "game_id": game.id(),
                "game_ended": game.is_game_ended(),
                "bingo_reached": game.is_bingo_reached(),
                "pouch_empty": game.is_pouch_empty()
            })))
        }
        Err(dump_error) => {
            log(LogLevel::Error, MODULE_NAME, &format!("Manual game dump failed: {dump_error}"));
            Err(ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to dump game: {dump_error}")))
        }
    }
}

pub async fn handle_global_gameslist(
    State(app_state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    // Check for optional client ID in headers
    let client_id_opt = if let Some(header_value) = headers.get("X-Client-ID") {
        if let Ok(id) = header_value.to_str() {
            Some(id.to_string())
        } else {
            None
        }
    } else {
        None
    };

    // Log with client ID if available
    if let Some(client_id) = &client_id_opt {
        log(LogLevel::Info, MODULE_NAME, &format!("[Client: {client_id}] Games list request"));
    } else {
        log(LogLevel::Info, MODULE_NAME, "Games list request");
    }

    // Get all games from the registry
    let games_result = app_state.game_registry.games_list();

    match games_result {
        Ok(games_list) => {
            let mut formatted_games = Vec::new();

            for (game_id, status, _info) in games_list {
                // Get the specific game to access its timestamps
                if let Ok(Some(game)) = app_state.game_registry.get_game(&game_id) {
                    // Get the GameEntry to access closed_at information
                    // Since we can't directly access GameEntry, we'll use the info from games_list
                    let games_for_details = app_state.game_registry.games_list().unwrap_or_default();
                    let game_details = games_for_details.iter().find(|(id, _, _)| id == &game_id);

                    // Extract closed_at from the info string if present
                    let closed_at = if let Some((_, _, info)) = game_details {
                        if info.contains("closed_at=") {
                            let parts: Vec<&str> = info.split("closed_at=").collect();
                            if parts.len() > 1 {
                                let closed_part = parts[1].split(']').next().unwrap_or("");
                                if closed_part.is_empty() {
                                    None
                                } else {
                                    Some(closed_part.to_string())
                                }
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    };

                    formatted_games.push(json!({
                        "game_id": game_id,
                        "status": status.as_str(),
                        "start_date": game.created_at_string(),
                        "close_date": closed_at
                    }));
                }
            }

            // Get registry statistics
            let (new_count, active_count, closed_count) = app_state.game_registry.status_summary()
                .unwrap_or((0, 0, 0));
            let total_games = app_state.game_registry.total_games().unwrap_or(0);

            Ok(Json(json!({
                "success": true,
                "total_games": total_games,
                "statistics": {
                    "new_games": new_count,
                    "active_games": active_count,
                    "closed_games": closed_count
                },
                "games": formatted_games
            })))
        }
        Err(error) => {
            log(LogLevel::Error, MODULE_NAME, &format!("Failed to get games list: {error}"));
            Err(ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to get games list: {error}")))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
        use std::sync::Arc;
        use crate::client::ClientRegistry;

        let config = ServerConfig::default();
        let game_registry = crate::game::GameRegistry::new();
        Arc::new(AppState {
            game_registry,
            global_client_registry: ClientRegistry::new(),
            config
        })
    }

    // Helper function to create a test game via API and return its ID
    async fn create_test_game(app_state: &Arc<AppState>) -> String {
        // Use the board client to create a new game via API
        let mut board_headers = HeaderMap::new();
        board_headers.insert("X-Client-ID", BOARD_ID.parse().unwrap());

        let newgame_result = handle_global_newgame(State(app_state.clone()), board_headers).await;
        match newgame_result {
            Ok(response) => {
                let game_id = response["game_id"].as_str().unwrap().to_string();

                // Register BOARD_ID as a board client for this new game using API
                register_board_client_to_game(app_state, &game_id).await;

                game_id
            }
            Err(_) => panic!("Failed to create test game via API"),
        }
    }

    // Helper function to get test game ID (creates a new game for testing)
    async fn get_test_game_id(app_state: &Arc<AppState>) -> String {
        create_test_game(app_state).await
    }

    // Helper function to register the board client to a game for testing using API handlers
    async fn register_board_client_to_game(app_state: &Arc<AppState>, game_id: &str) {
        // First, manually add the board client to the global registry with the fixed BOARD_ID
        // This ensures the board client has the expected ID constant
        let board_client_info = crate::client::ClientInfo {
            id: BOARD_ID.to_string(),
            name: "Board".to_string(),
            client_type: "board".to_string(),
            registered_at: std::time::SystemTime::now(),
            email: String::new(),
        };

        // Add to global registry (this will not overwrite if already exists)
        let _ = app_state.global_client_registry.insert(board_client_info);

        // Now register the board client to the specific game using the API
        let board_register_request = RegisterRequest {
            name: "Board".to_string(),
            client_type: "board".to_string(),
            nocard: Some(0), // Board doesn't need cards
            email: None,
        };

        let join_result = handle_join(
            Path(game_id.to_string()),
            State(app_state.clone()),
            JsonExtractor(board_register_request)
        ).await;

        // This should succeed since we're registering a board client
        assert!(join_result.is_ok(), "Failed to register board client to game {game_id}");
        let join_response = join_result.unwrap();

        // Verify the board client got the expected BOARD_ID
        assert_eq!(join_response.client_id, BOARD_ID, "Board client should have BOARD_ID");
    }

    // Helper function to create a registered client
    async fn register_test_client(app_state: &Arc<AppState>, name: &str) -> String {
        let request = RegisterRequest {
            name: name.to_string(),
            client_type: "player".to_string(),
            nocard: Some(1),
            email: None,
        };

        let game_id = create_test_game(app_state).await; // Create a test game
        let result = handle_join(Path(game_id), State(app_state.clone()), JsonExtractor(request)).await;
        match result {
            Ok(response) => response.0.client_id,
            Err(_) => panic!("Failed to register test client"),
        }
    }

    // Helper function to create a registered client in a specific game
    async fn register_test_client_to_game(app_state: &Arc<AppState>, name: &str, game_id: &str) -> String {
        let request = RegisterRequest {
            name: name.to_string(),
            client_type: "player".to_string(),
            nocard: Some(1),
            email: None,
        };

        let result = handle_join(Path(game_id.to_string()), State(app_state.clone()), JsonExtractor(request)).await;
        match result {
            Ok(response) => response.0.client_id,
            Err(_) => panic!("Failed to register test client to game"),
        }
    }

    #[tokio::test]
    async fn test_handle_register_new_client() {
        let app_state = create_test_app_state();
        let request = RegisterRequest {
            name: "test_player".to_string(),
            client_type: "player".to_string(),
            nocard: Some(2),
            email: None,
        };

        let game_id = create_test_game(&app_state).await;
        let result = handle_join(Path(game_id.clone()), State(app_state.clone()), JsonExtractor(request)).await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.message, format!("Client 'test_player' registered successfully in game '{game_id}'"));
        assert!(!response.client_id.is_empty());
    }

    #[tokio::test]
    async fn test_handle_register_existing_client() {
        let app_state = create_test_app_state();
        let request = RegisterRequest {
            name: "existing_player".to_string(),
            client_type: "player".to_string(),
            nocard: Some(1),
            email: None,
        };

        let game_id = create_test_game(&app_state).await;

        // Register the client first time
        let first_result = handle_join(Path(game_id.clone()), State(app_state.clone()), JsonExtractor(request.clone())).await;
        assert!(first_result.is_ok());
        let first_response = first_result.unwrap();

        // Try to register the same client again
        let second_result = handle_join(Path(game_id.clone()), State(app_state.clone()), JsonExtractor(request)).await;
        assert!(second_result.is_ok());
        let second_response = second_result.unwrap();

        assert_eq!(first_response.client_id, second_response.client_id);
        assert_eq!(second_response.message, format!("Client 'existing_player' already registered in game '{game_id}'"));
    }

    #[tokio::test]
    async fn test_handle_register_after_game_started() {
        let app_state = create_test_app_state();

        // Start the game by extracting a number through the API
        let game_id = create_test_game(&app_state).await;

        // Register BOARD_ID as a board client for this game using API
        register_board_client_to_game(&app_state, &game_id).await;

        let mut board_headers = HeaderMap::new();
        board_headers.insert("X-Client-ID", BOARD_ID.parse().unwrap());

        let _ = handle_extract(
            State(app_state.clone()),
            Path(game_id.clone()),
            board_headers,
            Query(ClientIdQuery { client_id: None }),
        ).await.unwrap();

        let request = RegisterRequest {
            name: "late_player".to_string(),
            client_type: "player".to_string(),
            nocard: Some(1),
            email: None,
        };

        let result = handle_join(Path(game_id), State(app_state.clone()), JsonExtractor(request)).await;

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

        let result = handle_global_clientinfo(
            State(app_state.clone()),
            HeaderMap::new(),
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

        let result = handle_global_clientinfo(
            State(app_state.clone()),
            HeaderMap::new(),
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

        let result = handle_global_clientinfo_by_id(
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

        let result = handle_global_clientinfo_by_id(
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

        let result = handle_global_clientinfo_by_id(
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
        let _game_id = get_test_game_id(&app_state).await;

        // Register a client with no cards during registration
        let register_request = RegisterRequest {
            name: "cards_test_player".to_string(),
            client_type: "player".to_string(),
            nocard: Some(0), // No cards during registration
            email: None,
        };

        let game_id = create_test_game(&app_state).await;
        let register_result = handle_join(Path(game_id.clone()), State(app_state.clone()), JsonExtractor(register_request)).await;
        assert!(register_result.is_ok());
        let client_id = register_result.unwrap().0.client_id;

        let mut headers = HeaderMap::new();
        headers.insert("X-Client-ID", client_id.parse().unwrap());

        let request = GenerateCardsRequest { count: 3 };

        let result = handle_generatecards(
            State(app_state.clone()),
            Path(game_id),
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
        let game_id = get_test_game_id(&app_state).await;
        let headers = HeaderMap::new(); // No X-Client-ID header

        let request = GenerateCardsRequest { count: 1 };

        let result = handle_generatecards(
            State(app_state.clone()),
            Path(game_id),
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
        let game_id = get_test_game_id(&app_state).await;
        let mut headers = HeaderMap::new();
        headers.insert("X-Client-ID", "invalid_client_id".parse().unwrap());

        let request = GenerateCardsRequest { count: 1 };

        let result = handle_generatecards(
            State(app_state.clone()),
            Path(game_id),
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
        let game_id = get_test_game_id(&app_state).await;
        let client_id = register_test_client_to_game(&app_state, "list_test_player", &game_id).await;

        let mut headers = HeaderMap::new();
        headers.insert("X-Client-ID", client_id.parse().unwrap());

        let result = handle_listassignedcards(
            State(app_state.clone()),
            Path(game_id),
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
        let game_id = get_test_game_id(&app_state).await;
        let headers = HeaderMap::new(); // No X-Client-ID header

        let result = handle_listassignedcards(
            State(app_state.clone()),
            Path(game_id),
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
        let game_id = get_test_game_id(&app_state).await;
        let client_id = register_test_client_to_game(&app_state, "get_card_test_player", &game_id).await;

        // Get the assigned card ID
        let mut headers = HeaderMap::new();
        headers.insert("X-Client-ID", client_id.parse().unwrap());

        let list_result = handle_listassignedcards(
            State(app_state.clone()),
            Path(game_id.clone()),
            headers.clone(),
            Query(ClientIdQuery { client_id: None }),
        ).await;

        assert!(list_result.is_ok());
        let list_response = list_result.unwrap();
        assert!(!list_response.0.cards.is_empty());

        let card_id = &list_response.0.cards[0].card_id;

        let result = handle_getassignedcard(
            State(app_state.clone()),
            Path((game_id, card_id.clone())),
            headers,
        ).await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.0["card_id"], *card_id);
    }

    #[tokio::test]
    async fn test_handle_get_assigned_card_not_found() {
        let app_state = create_test_app_state();
        let game_id = get_test_game_id(&app_state).await;
        let client_id = register_test_client_to_game(&app_state, "get_card_test_player", &game_id).await;

        let mut headers = HeaderMap::new();
        headers.insert("X-Client-ID", client_id.parse().unwrap());

        let result = handle_getassignedcard(
            State(app_state.clone()),
            Path((game_id, "nonexistent_card_id".to_string())),
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
        let game_id = get_test_game_id(&app_state).await;

        let result = handle_board(
            State(app_state.clone()),
            Path(game_id),
            HeaderMap::new(),
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
        let game_id = get_test_game_id(&app_state).await;

        // Register BOARD_ID as a board client for this game using API
        register_board_client_to_game(&app_state, &game_id).await;

        // Extract some numbers using the proper API handler
        let mut headers = HeaderMap::new();
        headers.insert("X-Client-ID", BOARD_ID.parse().unwrap()); // Board client

        let _ = handle_extract(
            State(app_state.clone()),
            Path(game_id.clone()),
            headers.clone(),
            Query(ClientIdQuery { client_id: None }),
        ).await.unwrap();

        let _ = handle_extract(
            State(app_state.clone()),
            Path(game_id.clone()),
            headers,
            Query(ClientIdQuery { client_id: None }),
        ).await.unwrap();

        let result = handle_board(
            State(app_state.clone()),
            Path(game_id),
            HeaderMap::new(),
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
        let game_id = get_test_game_id(&app_state).await;

        let result = handle_pouch(
            State(app_state.clone()),
            Path(game_id),
            HeaderMap::new(),
            Query(ClientIdQuery { client_id: None }),
        ).await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.0["numbers"].as_array().unwrap().len(), 90); // Full pouch
    }

    #[tokio::test]
    async fn test_handle_pouch_after_extraction() {
        let app_state = create_test_app_state();
        let game_id = get_test_game_id(&app_state).await;

        // Register BOARD_ID as a board client for this game using API
        register_board_client_to_game(&app_state, &game_id).await;

        // Extract a number through the API
        let mut board_headers = HeaderMap::new();
        board_headers.insert("X-Client-ID", BOARD_ID.parse().unwrap());

        let _ = handle_extract(
            State(app_state.clone()),
            Path(game_id.clone()),
            board_headers,
            Query(ClientIdQuery { client_id: None }),
        ).await.unwrap();

        let result = handle_pouch(
            State(app_state.clone()),
            Path(game_id),
            HeaderMap::new(),
            Query(ClientIdQuery { client_id: None }),
        ).await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.0["numbers"].as_array().unwrap().len(), 89); // One less after extraction
    }

    #[tokio::test]
    async fn test_handle_scoremap_initial_state() {
        let app_state = create_test_app_state();
        let game_id = get_test_game_id(&app_state).await;

        let result = handle_scoremap(
            State(app_state.clone()),
            Path(game_id),
            HeaderMap::new(),
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
        let game_id = get_test_game_id(&app_state).await;

        let result = handle_status(
            State(app_state.clone()),
            Path(game_id),
            HeaderMap::new(),
            Query(ClientIdQuery { client_id: None }),
        ).await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.0["status"], "new");  // New game should have "new" status
        assert!(response.0["game_id"].is_string());
        assert!(response.0["created_at"].is_string());
        assert_eq!(response.0["numbers_extracted"], 0);
        assert_eq!(response.0["scorecard"], 0);
        // Note: server field was removed from new implementation, so don't check it
    }

    #[tokio::test]
    async fn test_handle_extract_success() {
        let app_state = create_test_app_state();
        let game_id = get_test_game_id(&app_state).await;

        // Register BOARD_ID as a board client for this game using API
        register_board_client_to_game(&app_state, &game_id).await;

        let mut headers = HeaderMap::new();
        headers.insert("X-Client-ID", BOARD_ID.parse().unwrap()); // Board client

        let result = handle_extract(
            State(app_state.clone()),
            Path(game_id),
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
        let game_id = get_test_game_id(&app_state).await;
        let client_id = register_test_client_to_game(&app_state, "extract_test_player", &game_id).await;

        let mut headers = HeaderMap::new();
        headers.insert("X-Client-ID", client_id.parse().unwrap()); // Regular client, not board

        let result = handle_extract(
            State(app_state.clone()),
            Path(game_id),
            headers,
            Query(ClientIdQuery { client_id: None }),
        ).await;

        assert!(result.is_err());
        let error = result.unwrap_err();
        assert_eq!(error.status, StatusCode::FORBIDDEN);
        assert!(error.message.contains("Unauthorized: Only board clients can extract numbers"));
    }

    #[tokio::test]
    async fn test_handle_extract_missing_client_id() {
        let app_state = create_test_app_state();
        let game_id = get_test_game_id(&app_state).await;
        let headers = HeaderMap::new(); // No X-Client-ID header

        let result = handle_extract(
            State(app_state.clone()),
            Path(game_id),
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
        let game_id = get_test_game_id(&app_state).await;

        // Register BOARD_ID as a board client for the original game using API
        register_board_client_to_game(&app_state, &game_id).await;

        let mut headers = HeaderMap::new();
        headers.insert("X-Client-ID", BOARD_ID.parse().unwrap()); // Board client

        // Register a client and extract some numbers to have game state through API
        let _ = register_test_client(&app_state, "newgame_test_player").await;

        let _ = handle_extract(
            State(app_state.clone()),
            Path(game_id),
            headers.clone(),
            Query(ClientIdQuery { client_id: None }),
        ).await.unwrap();

        // Get initial game count
        let initial_count = app_state.game_registry.total_games().unwrap();

        let result = handle_global_newgame(State(app_state.clone()), headers).await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response["success"], true);
        assert_eq!(response["message"], "New game created successfully");
        assert!(response["game_id"].is_string());
        assert!(response["created_at"].is_string());
        assert!(response["note"].is_string());

        // Verify a new game was added to the registry
        let final_count = app_state.game_registry.total_games().unwrap();
        assert_eq!(final_count, initial_count + 1);
    }

    #[tokio::test]
    async fn test_handle_newgame_unauthorized() {
        let app_state = create_test_app_state();
        let client_id = register_test_client(&app_state, "newgame_test_player").await;

        let mut headers = HeaderMap::new();
        headers.insert("X-Client-ID", client_id.parse().unwrap()); // Regular client, not board

        let result = handle_global_newgame(State(app_state.clone()), headers).await;

        assert!(result.is_err());
        let error = result.unwrap_err();
        assert_eq!(error.status, StatusCode::FORBIDDEN);
        assert!(error.message.contains("Unauthorized: Only board client can create a new game"));
    }

    #[tokio::test]
    async fn test_handle_dumpgame_success() {
        let app_state = create_test_app_state();
        let game_id = get_test_game_id(&app_state).await;

        // Register BOARD_ID as a board client for this game using API
        register_board_client_to_game(&app_state, &game_id).await;

        let mut headers = HeaderMap::new();
        headers.insert("X-Client-ID", BOARD_ID.parse().unwrap()); // Board client

        // Create some game state through API
        let _ = register_test_client(&app_state, "dumpgame_test_player").await;

        let _ = handle_extract(
            State(app_state.clone()),
            Path(game_id.clone()),
            headers.clone(),
            Query(ClientIdQuery { client_id: None }),
        ).await.unwrap();

        let result = handle_dumpgame(State(app_state.clone()), Path(game_id), headers).await;

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
        let game_id = get_test_game_id(&app_state).await;
        let client_id = register_test_client(&app_state, "dumpgame_test_player").await;

        let mut headers = HeaderMap::new();
        headers.insert("X-Client-ID", client_id.parse().unwrap()); // Regular client, not board

        let result = handle_dumpgame(State(app_state.clone()), Path(game_id), headers).await;

        assert!(result.is_err());
        let error = result.unwrap_err();
        assert_eq!(error.status, StatusCode::FORBIDDEN);
        assert!(error.message.contains("Unauthorized: Only board clients can dump the game"));
    }

    #[tokio::test]
    async fn test_handle_dumpgame_missing_client_id() {
        let app_state = create_test_app_state();
        let game_id = get_test_game_id(&app_state).await;
        let headers = HeaderMap::new(); // No X-Client-ID header

        let result = handle_dumpgame(State(app_state.clone()), Path(game_id), headers).await;

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
        let game_id = get_test_game_id(&app_state).await;

        // Register BOARD_ID as a board client for this game using API
        register_board_client_to_game(&app_state, &game_id).await;

        // Register a client
        let client_id = register_test_client_to_game(&app_state, "integration_test_player", &game_id).await;

        // List assigned cards
        let mut headers = HeaderMap::new();
        headers.insert("X-Client-ID", client_id.parse().unwrap());

        let cards_result = handle_listassignedcards(
            State(app_state.clone()),
            Path(game_id.clone()),
            headers.clone(),
            Query(ClientIdQuery { client_id: None }),
        ).await;
        assert!(cards_result.is_ok());
        let cards = cards_result.unwrap();
        assert_eq!(cards.0.cards.len(), 1);

        // Get specific card
        let card_id = &cards.0.cards[0].card_id;
        let card_result = handle_getassignedcard(
            State(app_state.clone()),
            Path((game_id.clone(), card_id.clone())),
            headers.clone(),
        ).await;
        assert!(card_result.is_ok());

        // Extract a number (as board client)
        let mut board_headers = HeaderMap::new();
        board_headers.insert("X-Client-ID", BOARD_ID.parse().unwrap());

        let extract_result = handle_extract(
            State(app_state.clone()),
            Path(game_id.clone()),
            board_headers,
            Query(ClientIdQuery { client_id: None }),
        ).await;
        assert!(extract_result.is_ok());

        // Check board state
        let board_result = handle_board(
            State(app_state.clone()),
            Path(game_id.clone()),
            HeaderMap::new(),
            Query(ClientIdQuery { client_id: None }),
        ).await;
        assert!(board_result.is_ok());
        let board = board_result.unwrap();
        assert_eq!(board.0["numbers"].as_array().unwrap().len(), 1);

        // Check status
        let status_result = handle_status(
            State(app_state.clone()),
            Path(game_id),
            HeaderMap::new(),
            Query(ClientIdQuery { client_id: None }),
        ).await;
        assert!(status_result.is_ok());
        let status = status_result.unwrap();
        assert_eq!(status.0["numbers_extracted"], 1);
    }

    #[tokio::test]
    async fn test_handle_global_gameslist() {
        let app_state = create_test_app_state();

        // Create a test game first
        let _game_id = create_test_game(&app_state).await;

        // Test the games list endpoint
        let empty_headers = HeaderMap::new();
        let result = handle_global_gameslist(State(app_state.clone()), empty_headers).await;
        assert!(result.is_ok());

        let response = result.unwrap();
        let json_value = &response.0;

        // Verify response structure
        assert_eq!(json_value["success"], true);
        assert_eq!(json_value["total_games"], 1);

        // Check statistics (should be 1 new game)
        let stats = &json_value["statistics"];
        assert_eq!(stats["new_games"], 1);
        assert_eq!(stats["active_games"], 0);
        assert_eq!(stats["closed_games"], 0);

        // Check games array
        let games = json_value["games"].as_array().unwrap();
        assert_eq!(games.len(), 1);

        let game = &games[0];
        assert!(game["game_id"].as_str().unwrap().starts_with("game_"));
        assert_eq!(game["status"], "New");
        assert!(game["start_date"].as_str().unwrap().contains("UTC"));
        assert_eq!(game["close_date"], serde_json::Value::Null);
    }

    #[tokio::test]
    async fn test_handle_games_list_with_multiple_games() {
        let app_state = create_test_app_state();

        // Create a new game first via API
        let _new_game_id = create_test_game(&app_state).await;

        // Create an active game via API and make it active by extracting numbers
        let active_game_id = create_test_game(&app_state).await;
        register_board_client_to_game(&app_state, &active_game_id).await;

        let mut board_headers = HeaderMap::new();
        board_headers.insert("X-Client-ID", BOARD_ID.parse().unwrap());

        // Extract a number to make it active
        let _ = handle_extract(
            State(app_state.clone()),
            Path(active_game_id),
            board_headers.clone(),
            Query(ClientIdQuery { client_id: None }),
        ).await.unwrap();

        // Test the games list endpoint - we now have 2 games (1 new, 1 active)
        let empty_headers = HeaderMap::new();
        let result = handle_global_gameslist(State(app_state.clone()), empty_headers).await;
        assert!(result.is_ok());

        let response = result.unwrap();
        let json_value = &response.0;

        // Verify response structure
        assert_eq!(json_value["success"], true);
        assert_eq!(json_value["total_games"], 2); // New + active

        // Check statistics
        let stats = &json_value["statistics"];
        assert_eq!(stats["new_games"], 1);
        assert_eq!(stats["active_games"], 1);
        assert_eq!(stats["closed_games"], 0); // No closed games yet

        // Check games array
        let games = json_value["games"].as_array().unwrap();
        assert_eq!(games.len(), 2);

        // Verify we have the expected statuses represented
        let statuses: Vec<&str> = games.iter()
            .map(|game| game["status"].as_str().unwrap())
            .collect();
        assert!(statuses.contains(&"New"));
        assert!(statuses.contains(&"Active"));

        // Verify the active game doesn't have a close_date
        let active_games: Vec<&serde_json::Value> = games.iter()
            .filter(|game| game["status"] == "Active")
            .collect();
        assert_eq!(active_games.len(), 1);

        let active_game_data = active_games[0];
        assert_eq!(active_game_data["close_date"], serde_json::Value::Null);
    }

    #[tokio::test]
    async fn test_multi_game_scenario_with_clients_and_bingo() {
        println!("🎲 Starting comprehensive multi-game test with 3 games, 2 clients each, 6 cards per client");

        let app_state = create_test_app_state();

        // Register BOARD_ID as a board client for the original game using API
        let original_game_id = get_test_game_id(&app_state).await;
        register_board_client_to_game(&app_state, &original_game_id).await;

        // Step 1: Create 3 new games via the newgame endpoint
        let mut game_ids = Vec::new();
        let mut board_headers = HeaderMap::new();
        board_headers.insert("X-Client-ID", BOARD_ID.parse().unwrap());

        for i in 1..=3 {
            println!("📝 Creating game {i}");
            let newgame_result = handle_global_newgame(State(app_state.clone()), board_headers.clone()).await;
            assert!(newgame_result.is_ok(), "Failed to create game {i}");

            let newgame_response = newgame_result.unwrap();
            let game_id = newgame_response["game_id"].as_str().unwrap().to_string();

            // Register BOARD_ID as a board client for this new game using API
            register_board_client_to_game(&app_state, &game_id).await;

            game_ids.push(game_id.clone());
            println!("✅ Created game {i} with ID: {game_id}");
        }

        // Verify we have 4 games total (original + 3 new)
        let empty_headers = HeaderMap::new();
        let games_list_result = handle_global_gameslist(State(app_state.clone()), empty_headers).await;
        assert!(games_list_result.is_ok());
        let games_list_response = games_list_result.unwrap();
        assert_eq!(games_list_response["total_games"], 4);
        println!("🎯 Verified total games count: 4");

        // Step 2: For each game, register clients through the API and verify card generation
        let mut game_client_data = Vec::new();

        for (game_index, game_id) in game_ids.iter().enumerate() {
            println!("\n🎮 Setting up clients for game {} (ID: {game_id})", game_index + 1);
            let mut clients_for_game = Vec::new();

            for client_index in 1..=2 {
                let client_name = format!("player_game{}_client{}", game_index + 1, client_index);
                println!("👤 Registering client through API: {client_name}");

                // Register client through the proper API with 6 cards
                let register_request = RegisterRequest {
                    name: client_name.clone(),
                    client_type: "player".to_string(),
                    nocard: Some(6), // Request 6 cards during registration
                    email: None,
                };

                let register_result = handle_join(
                    Path(game_id.clone()),
                    State(app_state.clone()),
                    JsonExtractor(register_request)
                ).await;

                assert!(register_result.is_ok(), "Failed to register {client_name} in game {game_id}");
                let register_response = register_result.unwrap();
                let client_id = register_response.client_id.clone();

                println!("✅ Registered {client_name} with ID: {client_id} in game {} via API", game_index + 1);
                println!("📝 Registration message: {}", register_response.message);

                // Verify registration was successful and cards were assigned
                assert!(register_response.message.contains(&format!("registered successfully in game '{game_id}'")));

                // Verify cards are assigned to this client in this game using API
                let mut client_headers = HeaderMap::new();
                client_headers.insert("X-Client-ID", client_id.parse().unwrap());

                let list_cards_result = handle_listassignedcards(
                    State(app_state.clone()),
                    Path(game_id.clone()),
                    client_headers.clone(),
                    Query(ClientIdQuery { client_id: None }),
                ).await;

                assert!(list_cards_result.is_ok(), "Failed to list cards for {client_name} in game {game_id}");
                let cards_response = list_cards_result.unwrap();
                assert_eq!(cards_response.cards.len(), 6, "Expected 6 cards for {client_name} registered via API");
                println!("🃏 Verified 6 cards assigned to {client_name} in game {} via registration", game_index + 1);

                // Get one card to verify it belongs to this client
                let card_id = &cards_response.cards[0].card_id;
                let get_card_result = handle_getassignedcard(
                    State(app_state.clone()),
                    Path((game_id.clone(), card_id.clone())),
                    client_headers,
                ).await;

                assert!(get_card_result.is_ok(), "Failed to get card {card_id} for {client_name} in game {game_id}");
                println!("🔍 Verified card access for {client_name} in game {}", game_index + 1);

                clients_for_game.push((client_name, client_id));
            }

            game_client_data.push((game_id.clone(), clients_for_game));
        }

        // Step 3: Extract numbers in each game until BINGO is reached
        println!("\n🎯 Starting number extraction phase for all games");

        for (game_index, (game_id, clients)) in game_client_data.iter().enumerate() {
            println!("\n🎲 Extracting numbers for game {} (ID: {game_id})", game_index + 1);

            let mut extraction_count = 0;
            let max_extractions = 90; // Safety limit

            loop {
                // Check current game state via board
                let board_result = handle_board(
                    State(app_state.clone()),
                    Path(game_id.clone()),
                    HeaderMap::new(),
                    Query(ClientIdQuery { client_id: None }),
                ).await;

                assert!(board_result.is_ok(), "Failed to get board for game {game_id}");
                let board_response = board_result.unwrap();
                let numbers_extracted = board_response["numbers"].as_array().unwrap().len();

                println!("📊 Game {}: {} numbers extracted so far", game_index + 1, numbers_extracted);

                // Try to extract a number
                let extract_result = handle_extract(
                    State(app_state.clone()),
                    Path(game_id.clone()),
                    board_headers.clone(),
                    Query(ClientIdQuery { client_id: None }),
                ).await;

                extraction_count += 1;

                if extract_result.is_ok() {
                    let extract_response = extract_result.unwrap();
                    let extracted_number = extract_response["extracted_number"].as_i64().unwrap();
                    let remaining = extract_response["numbers_remaining"].as_i64().unwrap();
                    let total = extract_response["total_extracted"].as_i64().unwrap();

                    println!("🎯 Game {}: Extracted number {} (Total: {}, Remaining: {})",
                             game_index + 1, extracted_number, total, remaining);
                } else {
                    let error = extract_result.unwrap_err();
                    if error.status == StatusCode::CONFLICT && error.message.contains("BINGO") {
                        println!("🏆 Game {}: BINGO REACHED! Game completed after {} extractions",
                                 game_index + 1, extraction_count - 1);
                        break;
                    }
                    panic!("Unexpected extraction error in game {}: {}", game_index + 1, error.message);
                }

                // Safety check to prevent infinite loops
                assert!((extraction_count < max_extractions), "Game {} exceeded maximum extractions without reaching BINGO", game_index + 1);
            }

            // Verify game is properly dumped and in closed state
            let final_status_result = handle_status(
                State(app_state.clone()),
                Path(game_id.clone()),
                HeaderMap::new(),
                Query(ClientIdQuery { client_id: None }),
            ).await;

            assert!(final_status_result.is_ok(), "Failed to get final status for game {game_id}");
            println!("✅ Game {} completed and verified", game_index + 1);

            // Test that clients can still access their cards in the completed game
            for (client_name, client_id) in clients {
                let mut client_headers = HeaderMap::new();
                client_headers.insert("X-Client-ID", client_id.parse().unwrap());

                let final_cards_result = handle_listassignedcards(
                    State(app_state.clone()),
                    Path(game_id.clone()),
                    client_headers,
                    Query(ClientIdQuery { client_id: None }),
                ).await;

                assert!(final_cards_result.is_ok(), "Failed to access cards for {client_name} in completed game");
                let final_cards = final_cards_result.unwrap();
                assert_eq!(final_cards.cards.len(), 6, "Client {client_name} should still have 6 cards");
            }

            println!("🔍 Verified client access to completed game {}", game_index + 1);
        }

        // Step 4: Final verification - check games list shows all games as closed
        println!("\n📋 Final verification of all games");

        let empty_headers = HeaderMap::new();
        let final_games_list = handle_global_gameslist(State(app_state.clone()), empty_headers).await;
        assert!(final_games_list.is_ok());
        let final_response = final_games_list.unwrap();

        let final_games = final_response["games"].as_array().unwrap();
        let closed_games_count = final_games.iter()
            .filter(|game| game["status"] == "Closed")
            .count();

        // We should have at least 3 closed games (the ones we completed)
        assert!(closed_games_count >= 3, "Expected at least 3 closed games, found {closed_games_count}");

        let stats = &final_response["statistics"];
        println!("📈 Final statistics:");
        println!("   • Total games: {}", final_response["total_games"]);
        println!("   • New games: {}", stats["new_games"]);
        println!("   • Active games: {}", stats["active_games"]);
        println!("   • Closed games: {}", stats["closed_games"]);

        // Verify each of our created games is in closed state
        for (game_index, game_id) in game_ids.iter().enumerate() {
            let game_found = final_games.iter().any(|game| {
                game["game_id"].as_str() == Some(game_id) && game["status"] == "Closed"
            });
            assert!(game_found, "Game {} ({}) should be in closed state", game_index + 1, game_id);
        }

        println!("\n🎉 SUCCESS! Multi-game test completed:");
        println!("   ✅ Created 3 games");
        println!("   ✅ Registered 2 clients per game (6 total)");
        println!("   ✅ Generated 6 cards per client (36 total)");
        println!("   ✅ Extracted numbers until BINGO in all 3 games");
        println!("   ✅ Verified client isolation per game");
        println!("   ✅ Verified all games reached completion");
    }

    #[tokio::test]
    async fn test_global_client_id_across_multiple_games() {
        println!("🌐 Testing global client ID functionality across multiple games");

        let app_state = create_test_app_state();

        // Step 1: Create two new games
        let mut board_headers = HeaderMap::new();
        board_headers.insert("X-Client-ID", BOARD_ID.parse().unwrap());

        // Create first new game
        let newgame1_result = handle_global_newgame(State(app_state.clone()), board_headers.clone()).await;
        assert!(newgame1_result.is_ok(), "Failed to create first game");
        let game1_id = newgame1_result.unwrap()["game_id"].as_str().unwrap().to_string();
        println!("✅ Created first game: {game1_id}");

        // Create second new game
        let newgame2_result = handle_global_newgame(State(app_state.clone()), board_headers.clone()).await;
        assert!(newgame2_result.is_ok(), "Failed to create second game");
        let game2_id = newgame2_result.unwrap()["game_id"].as_str().unwrap().to_string();
        println!("✅ Created second game: {game2_id}");

        // Step 2: Register the same client to the first game
        let client_name = "global_test_player";
        let register_request = RegisterRequest {
            name: client_name.to_string(),
            client_type: "player".to_string(),
            nocard: Some(2),
            email: None,
        };

        let register1_result = handle_join(
            Path(game1_id.clone()),
            State(app_state.clone()),
            JsonExtractor(register_request.clone())
        ).await;

        assert!(register1_result.is_ok(), "Failed to register client to first game");
        let register1_response = register1_result.unwrap();
        let client_id_game1 = register1_response.client_id.clone();
        println!("👤 Registered {client_name} to game1 with ID: {client_id_game1}");

        // Step 3: Register the same client to the second game
        let register2_result = handle_join(
            Path(game2_id.clone()),
            State(app_state.clone()),
            JsonExtractor(register_request.clone())
        ).await;

        assert!(register2_result.is_ok(), "Failed to register client to second game");
        let register2_response = register2_result.unwrap();
        let client_id_game2 = register2_response.client_id.clone();
        println!("👤 Registered {client_name} to game2 with ID: {client_id_game2}");

        // Step 4: Verify that both registrations returned the same client ID
        assert_eq!(client_id_game1, client_id_game2,
                   "Client should have the same ID across different games");
        println!("🎯 VERIFIED: Client has same ID ({client_id_game1}) in both games");

        // Step 5: Test global clientinfo endpoint by name
        let client_info_by_name_query = ClientNameQuery {
            name: Some(client_name.to_string()),
        };

        let clientinfo_by_name_result = handle_global_clientinfo(
            State(app_state.clone()),
            HeaderMap::new(),
            Query(client_info_by_name_query),
        ).await;

        assert!(clientinfo_by_name_result.is_ok(), "Failed to get client info by name");
        let clientinfo_by_name_response = clientinfo_by_name_result.unwrap();
        assert_eq!(clientinfo_by_name_response.name, client_name);
        assert_eq!(clientinfo_by_name_response.client_id, client_id_game1);
        assert_eq!(clientinfo_by_name_response.client_type, "player");
        println!("🔍 VERIFIED: Global clientinfo by name works correctly");

        // Step 6: Test global clientinfo endpoint by ID
        let clientinfo_by_id_result = handle_global_clientinfo_by_id(
            State(app_state.clone()),
            Path(client_id_game1.clone()),
        ).await;

        assert!(clientinfo_by_id_result.is_ok(), "Failed to get client info by ID");
        let clientinfo_by_id_response = clientinfo_by_id_result.unwrap();
        assert_eq!(clientinfo_by_id_response.name, client_name);
        assert_eq!(clientinfo_by_id_response.client_id, client_id_game1);
        assert_eq!(clientinfo_by_id_response.client_type, "player");
        println!("🔍 VERIFIED: Global clientinfo by ID works correctly");

        // Step 7: Verify client can access cards in both games
        let mut client_headers = HeaderMap::new();
        client_headers.insert("X-Client-ID", client_id_game1.parse().unwrap());

        // Check cards in game1
        let cards1_result = handle_listassignedcards(
            State(app_state.clone()),
            Path(game1_id.clone()),
            client_headers.clone(),
            Query(ClientIdQuery { client_id: None }),
        ).await;

        assert!(cards1_result.is_ok(), "Failed to list cards in game1");
        let cards1_response = cards1_result.unwrap();
        assert_eq!(cards1_response.cards.len(), 2, "Client should have 2 cards in game1");
        println!("🃏 VERIFIED: Client has 2 cards in game1");

        // Check cards in game2
        let cards2_result = handle_listassignedcards(
            State(app_state.clone()),
            Path(game2_id.clone()),
            client_headers.clone(),
            Query(ClientIdQuery { client_id: None }),
        ).await;

        assert!(cards2_result.is_ok(), "Failed to list cards in game2");
        let cards2_response = cards2_result.unwrap();
        assert_eq!(cards2_response.cards.len(), 2, "Client should have 2 cards in game2");
        println!("🃏 VERIFIED: Client has 2 cards in game2");

        // Step 8: Register a different client to verify isolation
        let different_client_name = "different_test_player";
        let different_register_request = RegisterRequest {
            name: different_client_name.to_string(),
            client_type: "player".to_string(),
            nocard: Some(1),
            email: None,
        };

        let different_register_result = handle_join(
            Path(game1_id.clone()),
            State(app_state.clone()),
            JsonExtractor(different_register_request)
        ).await;

        assert!(different_register_result.is_ok(), "Failed to register different client");
        let different_client_id = different_register_result.unwrap().0.client_id;
        assert_ne!(different_client_id, client_id_game1,
                   "Different clients should have different IDs");
        println!("👥 VERIFIED: Different client has different ID: {different_client_id}");

        // Step 9: Test clientinfo endpoint with non-existent client
        let nonexistent_result = handle_global_clientinfo_by_id(
            State(app_state.clone()),
            Path("NONEXISTENT_ID".to_string()),
        ).await;

        assert!(nonexistent_result.is_err(), "Non-existent client should return error");
        let error = nonexistent_result.unwrap_err();
        assert_eq!(error.status, StatusCode::NOT_FOUND);
        assert!(error.message.contains("Client with ID 'NONEXISTENT_ID' not found"));
        println!("❌ VERIFIED: Non-existent client ID returns proper error");

        // Step 10: Verify both clients are in global registry
        let global_client_info_result = handle_global_clientinfo(
            State(app_state.clone()),
            HeaderMap::new(),
            Query(ClientNameQuery { name: Some(different_client_name.to_string()) }),
        ).await;

        assert!(global_client_info_result.is_ok(), "Different client should be found globally");
        let different_global_info = global_client_info_result.unwrap();
        assert_eq!(different_global_info.client_id, different_client_id);
        println!("🌐 VERIFIED: Both clients are accessible through global registry");

        println!("\n🎉 SUCCESS! Global client ID test completed:");
        println!("   ✅ Same client gets identical ID across multiple games");
        println!("   ✅ Global clientinfo by name works correctly");
        println!("   ✅ Global clientinfo by ID works correctly");
        println!("   ✅ Client can access resources in multiple games with same ID");
        println!("   ✅ Different clients get different IDs");
        println!("   ✅ Proper error handling for non-existent clients");
        println!("   ✅ Global client registry maintains all clients");
    }

    #[tokio::test]
    async fn test_handle_global_register_new_client() {
        let app_state = create_test_app_state();
        let request = RegisterRequest {
            name: "global_new_player".to_string(),
            client_type: "player".to_string(),
            nocard: Some(0), // Not used in global registration
            email: None,
        };

        let result = handle_global_register(State(app_state.clone()), HeaderMap::new(), JsonExtractor(request)).await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.message, "Client 'global_new_player' registered successfully globally");
        assert!(!response.client_id.is_empty());

        // Verify client was added to global registry
        let client_info = app_state.global_client_registry.get_by_name("global_new_player").unwrap();
        assert!(client_info.is_some());
        let client = client_info.unwrap();
        assert_eq!(client.name, "global_new_player");
        assert_eq!(client.client_type, "player");
        assert_eq!(client.email, ""); // Empty since None was provided
    }

    #[tokio::test]
    async fn test_handle_global_register_with_email() {
        let app_state = create_test_app_state();
        let request = RegisterRequest {
            name: "global_email_player".to_string(),
            client_type: "player".to_string(),
            nocard: Some(0),
            email: Some("test@example.com".to_string()),
        };

        let result = handle_global_register(State(app_state.clone()), HeaderMap::new(), JsonExtractor(request)).await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.message, "Client 'global_email_player' registered successfully globally");

        // Verify client was added to global registry with email
        let client_info = app_state.global_client_registry.get_by_name("global_email_player").unwrap();
        assert!(client_info.is_some());
        let client = client_info.unwrap();
        assert_eq!(client.name, "global_email_player");
        assert_eq!(client.client_type, "player");
        assert_eq!(client.email, "test@example.com");
    }

    #[tokio::test]
    async fn test_handle_global_register_existing_client() {
        let app_state = create_test_app_state();

        // First registration
        let request1 = RegisterRequest {
            name: "global_existing_player".to_string(),
            client_type: "player".to_string(),
            nocard: Some(0),
            email: Some("first@example.com".to_string()),
        };

        let result1 = handle_global_register(State(app_state.clone()), HeaderMap::new(), JsonExtractor(request1)).await;
        assert!(result1.is_ok());
        let response1 = result1.unwrap();
        let first_client_id = response1.client_id.clone();

        // Second registration with same name but different email
        let request2 = RegisterRequest {
            name: "global_existing_player".to_string(),
            client_type: "admin".to_string(), // Different type
            nocard: Some(0),
            email: Some("second@example.com".to_string()), // Different email
        };

        let result2 = handle_global_register(State(app_state.clone()), HeaderMap::new(), JsonExtractor(request2)).await;
        assert!(result2.is_ok());
        let response2 = result2.unwrap();

        // Should return the same client ID and indicate already registered
        assert_eq!(response2.client_id, first_client_id);
        assert_eq!(response2.message, "Client 'global_existing_player' already registered globally");

        // Verify original client info is preserved (not updated)
        let client_info = app_state.global_client_registry.get_by_name("global_existing_player").unwrap();
        assert!(client_info.is_some());
        let client = client_info.unwrap();
        assert_eq!(client.client_type, "player"); // Original type preserved
        assert_eq!(client.email, "first@example.com"); // Original email preserved
    }

    #[tokio::test]
    async fn test_handle_global_register_different_client_types() {
        let app_state = create_test_app_state();

        // Register a player
        let player_request = RegisterRequest {
            name: "test_player".to_string(),
            client_type: "player".to_string(),
            nocard: Some(0),
            email: None,
        };

        let player_result = handle_global_register(State(app_state.clone()), HeaderMap::new(), JsonExtractor(player_request)).await;
        assert!(player_result.is_ok());

        // Register an admin
        let admin_request = RegisterRequest {
            name: "test_admin".to_string(),
            client_type: "admin".to_string(),
            nocard: Some(0),
            email: Some("admin@company.com".to_string()),
        };

        let admin_result = handle_global_register(State(app_state.clone()), HeaderMap::new(), JsonExtractor(admin_request)).await;
        assert!(admin_result.is_ok());

        // Verify both are in registry with correct types
        let player_info = app_state.global_client_registry.get_by_name("test_player").unwrap().unwrap();
        assert_eq!(player_info.client_type, "player");
        assert_eq!(player_info.email, "");

        let admin_info = app_state.global_client_registry.get_by_name("test_admin").unwrap().unwrap();
        assert_eq!(admin_info.client_type, "admin");
        assert_eq!(admin_info.email, "admin@company.com");

        // Ensure they have different IDs
        assert_ne!(player_info.id, admin_info.id);
    }

    #[tokio::test]
    async fn test_handle_global_register_integration_with_game_registration() {
        let app_state = create_test_app_state();
        let game_id = create_test_game(&app_state).await;

        // Step 1: Register client globally first
        let global_request = RegisterRequest {
            name: "integration_test_player".to_string(),
            client_type: "player".to_string(),
            nocard: Some(0),
            email: Some("integration@test.com".to_string()),
        };

        let global_result = handle_global_register(State(app_state.clone()), HeaderMap::new(), JsonExtractor(global_request)).await;
        assert!(global_result.is_ok());
        let global_response = global_result.unwrap();
        let global_client_id = global_response.client_id.clone();

        // Step 2: Register the same client to a specific game
        let game_request = RegisterRequest {
            name: "integration_test_player".to_string(),
            client_type: "player".to_string(), // Same type
            nocard: Some(2), // Request cards for game
            email: None, // Different email (should be ignored)
        };

        let game_result = handle_join(Path(game_id.clone()), State(app_state.clone()), JsonExtractor(game_request)).await;
        assert!(game_result.is_ok());
        let game_response = game_result.unwrap();

        // Should use the same client ID from global registry
        assert_eq!(game_response.client_id, global_client_id);
        assert!(game_response.message.contains("registered successfully"));

        // Verify client info comes from global registry
        let client_info = app_state.global_client_registry.get_by_name("integration_test_player").unwrap().unwrap();
        assert_eq!(client_info.id, global_client_id);
        assert_eq!(client_info.email, "integration@test.com"); // Original email preserved

        // Verify client is registered in the game and has cards
        let game = app_state.game_registry.get_game(&game_id).unwrap().unwrap();
        assert!(game.contains_client(&global_client_id));

        // Check that cards were assigned during game registration
        if let Ok(manager) = game.card_manager().lock() {
            let client_cards = manager.get_client_cards(&global_client_id);
            assert!(client_cards.is_some());
            assert_eq!(client_cards.unwrap().len(), 2); // 2 cards requested
        }
    }

    #[tokio::test]
    async fn test_handle_extract_game_specific_client_types() {
        let app_state = create_test_app_state();

        // Create two games via API
        let game1_id = create_test_game(&app_state).await;
        let game2_id = create_test_game(&app_state).await;

        // Register TestPlayer as "player" in game1
        let player_request = RegisterRequest {
            name: "TestPlayer".to_string(),
            client_type: "player".to_string(),
            nocard: Some(1),
            email: None,
        };

        let game1_register_result = handle_join(
            Path(game1_id.clone()),
            State(app_state.clone()),
            JsonExtractor(player_request.clone())
        ).await;
        assert!(game1_register_result.is_ok());
        let client_id = game1_register_result.unwrap().0.client_id;

        // Register same TestPlayer as "board" in game2
        let board_request = RegisterRequest {
            name: "TestPlayer".to_string(),
            client_type: "board".to_string(),
            nocard: Some(1),
            email: None,
        };

        let game2_register_result = handle_join(
            Path(game2_id.clone()),
            State(app_state.clone()),
            JsonExtractor(board_request)
        ).await;
        assert!(game2_register_result.is_ok());
        assert_eq!(game2_register_result.unwrap().0.client_id, client_id); // Same client ID

        // Test extraction authorization
        let mut headers = HeaderMap::new();
        headers.insert("X-Client-ID", client_id.parse().unwrap());

        // TestPlayer should NOT be able to extract from game1 (where they are "player")
        let game1_extract_result = handle_extract(
            State(app_state.clone()),
            Path(game1_id),
            headers.clone(),
            Query(ClientIdQuery { client_id: None }),
        ).await;
        assert!(game1_extract_result.is_err());
        let error = game1_extract_result.unwrap_err();
        assert_eq!(error.status, StatusCode::FORBIDDEN);
        assert!(error.message.contains("Unauthorized: Only board clients can extract numbers"));

        // TestPlayer SHOULD be able to extract from game2 (where they are "board")
        let game2_extract_result = handle_extract(
            State(app_state.clone()),
            Path(game2_id),
            headers,
            Query(ClientIdQuery { client_id: None }),
        ).await;
        assert!(game2_extract_result.is_ok());
        let response = game2_extract_result.unwrap();
        assert_eq!(response.0["success"], true);
        assert!(response.0["extracted_number"].is_number());
    }

    #[tokio::test]
    async fn test_handle_dumpgame_game_specific_client_types() {
        let app_state = create_test_app_state();

        // Create two games via API
        let game1_id = create_test_game(&app_state).await;
        let game2_id = create_test_game(&app_state).await;

        // Register TestPlayer as "player" in game1 and "board" in game2
        let player_request = RegisterRequest {
            name: "TestPlayer".to_string(),
            client_type: "player".to_string(),
            nocard: Some(1),
            email: None,
        };

        let game1_register_result = handle_join(
            Path(game1_id.clone()),
            State(app_state.clone()),
            JsonExtractor(player_request)
        ).await;
        assert!(game1_register_result.is_ok());
        let client_id = game1_register_result.unwrap().0.client_id;

        let board_request = RegisterRequest {
            name: "TestPlayer".to_string(),
            client_type: "board".to_string(),
            nocard: Some(1),
            email: None,
        };

        let game2_register_result = handle_join(
            Path(game2_id.clone()),
            State(app_state.clone()),
            JsonExtractor(board_request)
        ).await;
        assert!(game2_register_result.is_ok());

        // Test dumpgame authorization
        let mut headers = HeaderMap::new();
        headers.insert("X-Client-ID", client_id.parse().unwrap());

        // TestPlayer should NOT be able to dump game1 (where they are "player")
        let game1_dump_result = handle_dumpgame(
            State(app_state.clone()),
            Path(game1_id),
            headers.clone(),
        ).await;
        assert!(game1_dump_result.is_err());
        let error = game1_dump_result.unwrap_err();
        assert_eq!(error.status, StatusCode::FORBIDDEN);
        assert!(error.message.contains("Unauthorized: Only board clients can dump the game"));

        // TestPlayer SHOULD be able to dump game2 (where they are "board")
        let game2_dump_result = handle_dumpgame(
            State(app_state.clone()),
            Path(game2_id),
            headers,
        ).await;
        assert!(game2_dump_result.is_ok());
        let response = game2_dump_result.unwrap();
        assert_eq!(response.0["success"], true);
    }
}
