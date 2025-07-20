use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::convert::Infallible;
use std::net::SocketAddr;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{body::Bytes, Request, Response, StatusCode, Method};
use hyper_util::rt::TokioIo;
use tokio::net::TcpListener;
use http_body_util::{Full, BodyExt};
use serde_json::json;

// Import Board from board module
use crate::board::{Board, BOARD_ID};
use crate::pouch::Pouch;
use crate::score::ScoreCard;
use crate::client::{RegisterRequest, RegisterResponse, ClientInfoResponse, ClientInfo};
use crate::card::{GenerateCardsRequest, GenerateCardsResponse, CardInfo, ListAssignedCardsResponse, AssignedCardInfo};
use crate::config::ServerConfig;
use crate::logging::{log_info, log_error, log_error_stderr, log_warning};
use crate::game::Game;

// Response structures for JSON serialization
#[derive(serde::Serialize)]
struct ErrorResponse {
    error: String,
}

// Start the HTTP server with Tokio
pub fn start_server(config: ServerConfig) -> (tokio::task::JoinHandle<()>, Arc<AtomicBool>) {
    let shutdown_signal = Arc::new(AtomicBool::new(false));
    let shutdown_clone = Arc::clone(&shutdown_signal);

    // Create the unified Game state container
    let game = Game::new();
    log_info(&format!("Created new game instance: {}", game.game_info()));

    let handle = tokio::spawn(async move {
        let addr = SocketAddr::from((config.host.parse::<std::net::IpAddr>().unwrap_or([127, 0, 0, 1].into()), config.port));
        let listener = match TcpListener::bind(&addr).await {
            Ok(listener) => listener,
            Err(e) => {
                log_error_stderr(&format!("Failed to start API server: {e}"));
                return;
            }
        };

        loop {
            // Check if shutdown was requested
            if shutdown_clone.load(Ordering::Relaxed) {
                break;
            }

            // Accept connections with a timeout
            let accept_result = tokio::time::timeout(
                std::time::Duration::from_millis(100),
                listener.accept()
            ).await;

            match accept_result {
                Ok(Ok((stream, _))) => {
                    let game_clone = game.clone();
                    let io = TokioIo::new(stream);

                    // Spawn a task to handle the connection
                    tokio::spawn(async move {
                        let service = service_fn(move |req| {
                            handle_request(req, game_clone.clone())
                        });

                        if let Err(err) = http1::Builder::new()
                            .serve_connection(io, service)
                            .await
                        {
                            log_error_stderr(&format!("Error serving connection: {err:?}"));
                        }
                    });
                }
                Ok(Err(e)) => {
                    log_error_stderr(&format!("Error accepting connection: {e}"));
                    break;
                }
                Err(_) => {
                    // Timeout occurred, continue to check shutdown signal
                }
            }
        }
        log_info("API Server shutting down...");
    });

    (handle, shutdown_signal)
}

// Handle HTTP requests asynchronously
async fn handle_request(
    req: Request<hyper::body::Incoming>,
    game: Game,
) -> Result<Response<Full<Bytes>>, Infallible> {
    let response = match (req.method(), req.uri().path()) {
        (&Method::POST, "/register") => {
            handle_register(req, &game).await
        }
        (&Method::GET, path) if path.starts_with("/client/") => {
            let client_name = &path[8..]; // Remove "/client/" prefix
            handle_client_info(client_name, &game).await
        }
        (&Method::GET, path) if path.starts_with("/clientbyid/") => {
            let client_id = &path[12..]; // Remove "/clientbyid/" prefix
            handle_client_info_by_id(client_id, &game).await
        }
        (&Method::POST, "/generatecardsforme") => {
            handle_generate_cards(req, &game).await
        }
        (&Method::GET, "/listassignedcards") => {
            handle_list_assigned_cards(req, &game).await
        }
        (&Method::GET, path) if path.starts_with("/getassignedcard/") => {
            let card_id = path[17..].to_string(); // Remove "/getassignedcard/" prefix
            handle_get_assigned_card(req, &game, card_id).await
        }
        (&Method::GET, "/board") => {
            handle_board(&game).await
        }
        (&Method::GET, "/pouch") => {
            handle_pouch(&game).await
        }
        (&Method::GET, "/scoremap") => {
            handle_scoremap(&game).await
        }
        (&Method::POST, "/extract") => {
            handle_extract(req, &game).await
        }
        (&Method::POST, "/newgame") => {
            handle_newgame(req, &game).await
        }
        (&Method::POST, "/dumpgame") => {
            handle_dumpgame(req, &game).await
        }
        (&Method::GET, "/status") => {
            handle_status(&game).await
        }
        (&Method::GET, "/runninggameid") => {
            handle_running_game_id(&game).await
        }
        _ => {
            handle_not_found().await
        }
    };

    Ok(response)
}

// Handle client registration
async fn handle_register(
    req: Request<hyper::body::Incoming>,
    game: &Game,
) -> Response<Full<Bytes>> {
    // Read the request body
    let body = match req.collect().await {
        Ok(body) => body.to_bytes(),
        Err(_) => {
            let error_response = ErrorResponse {
                error: "Failed to read request body".to_string(),
            };
            let body = serde_json::to_string(&error_response).unwrap_or_else(|_| "{}".to_string());
            return Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .header("Content-Type", "application/json")
                .header("Access-Control-Allow-Origin", "*")
                .body(Full::new(Bytes::from(body)))
                .unwrap();
        }
    };

    // Parse the registration request
    let register_request: RegisterRequest = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(_) => {
            let error_response = ErrorResponse {
                error: "Invalid JSON in request body".to_string(),
            };
            let body = serde_json::to_string(&error_response).unwrap_or_else(|_| "{}".to_string());
            return Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .header("Content-Type", "application/json")
                .header("Access-Control-Allow-Origin", "*")
                .body(Full::new(Bytes::from(body)))
                .unwrap();
        }
    };

    // Create client info first
    let client_info = ClientInfo::new(
        &register_request.name,
        &register_request.client_type,
    );
    let client_id = client_info.id.clone();

    // Check if client already exists and return existing info
    if let Ok(mut registry) = game.client_registry().lock() {
        if let Some(existing_client) = registry.get(&register_request.name) {
            let register_response = RegisterResponse {
                client_id: existing_client.id.clone(),
                message: format!("Client '{}' already registered", register_request.name),
            };
            let body = serde_json::to_string(&register_response).unwrap_or_else(|_| "{}".to_string());
            return Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "application/json")
                .header("Access-Control-Allow-Origin", "*")
                .body(Full::new(Bytes::from(body)))
                .unwrap();
        }

        // Check if numbers have been extracted using the game's convenience method
        let numbers_extracted = game.has_game_started();

        // Try to register the new client (will fail if numbers have been extracted)
        match registry.insert(register_request.name.clone(), client_info, numbers_extracted) {
            Ok(_) => {
                log_info(&format!("Client registered: {} (ID: {})", register_request.name, client_id));
            }
            Err(error_msg) => {
                let error_response = ErrorResponse {
                    error: error_msg,
                };
                let body = serde_json::to_string(&error_response).unwrap_or_else(|_| "{}".to_string());
                return Response::builder()
                    .status(StatusCode::CONFLICT)
                    .header("Content-Type", "application/json")
                    .header("Access-Control-Allow-Origin", "*")
                    .body(Full::new(Bytes::from(body)))
                    .unwrap();
            }
        }
    }

    // Check if client requested cards during registration, default to 1 if not specified
    let card_count = register_request.nocard.unwrap_or(1);
    log_info(&format!("Generating {} cards for client '{}' during registration", card_count, register_request.name));

    // Generate the requested number of cards using the card manager
    if let Ok(mut manager) = game.card_manager().lock() {
        manager.assign_cards(client_id.clone(), card_count);
        log_info(&format!("Generated and assigned {} cards to client '{}'", card_count, register_request.name));
    } else {
        log_warning(&format!("Failed to acquire card manager lock for client '{}'", register_request.name));
    }

    // Create response
    let register_response = RegisterResponse {
        client_id: client_id.clone(),
        message: format!("Client '{}' registered successfully", register_request.name),
    };

    let body = serde_json::to_string(&register_response).unwrap_or_else(|_| "{}".to_string());
    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/json")
        .header("Access-Control-Allow-Origin", "*")
        .body(Full::new(Bytes::from(body)))
        .unwrap()
}

// Function to get client information by name
async fn handle_client_info(
    client_name: &str,
    game: &Game,
) -> Response<Full<Bytes>> {
    // Look up client by name
    if let Ok(registry) = game.client_registry().lock() {
        if let Some(client_info) = registry.get(client_name) {
            let client_response = ClientInfoResponse {
                client_id: client_info.id.clone(),
                name: client_info.name.clone(),
                client_type: client_info.client_type.clone(),
                registered_at: format!("{:?}", client_info.registered_at),
            };

            let body = serde_json::to_string(&client_response).unwrap_or_else(|_| "{}".to_string());
            return Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "application/json")
                .header("Access-Control-Allow-Origin", "*")
                .body(Full::new(Bytes::from(body)))
                .unwrap();
        }
    }

    // Client not found
    let error_response = ErrorResponse {
        error: format!("Client '{client_name}' not found"),
    };
    let body = serde_json::to_string(&error_response).unwrap_or_else(|_| "{}".to_string());
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .header("Content-Type", "application/json")
        .header("Access-Control-Allow-Origin", "*")
        .body(Full::new(Bytes::from(body)))
        .unwrap()
}

// Function to get client information by ID
async fn handle_client_info_by_id(
    client_id: &str,
    game: &Game,
) -> Response<Full<Bytes>> {
    // Use ClientRegistry method to resolve client name (handles both special board case and regular clients)
    let client_name = if let Ok(registry) = game.client_registry().lock() {
        registry.get_client_name_by_id(client_id)
    } else {
        None
    }.unwrap_or_else(|| "Unknown".to_string());

    // Handle special case for board client ID
    if client_name == "Board" {
        let client_response = ClientInfoResponse {
            client_id: client_id.to_string(),
            name: "Board".to_string(),
            client_type: "board".to_string(),
            registered_at: "System".to_string(),
        };

        let body = serde_json::to_string(&client_response).unwrap_or_else(|_| "{}".to_string());
        return Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", "application/json")
            .header("Access-Control-Allow-Origin", "*")
            .body(Full::new(Bytes::from(body)))
            .unwrap();
    }

    // Handle regular clients - if CardAssignmentManager found a name, look up full client info
    if client_name != "Unknown" {
        if let Ok(registry) = game.client_registry().lock() {
            for client_info in registry.values() {
                if client_info.name == client_name {
                    let client_response = ClientInfoResponse {
                        client_id: client_info.id.clone(),
                        name: client_info.name.clone(),
                        client_type: client_info.client_type.clone(),
                        registered_at: format!("{:?}", client_info.registered_at),
                    };

                    let body = serde_json::to_string(&client_response).unwrap_or_else(|_| "{}".to_string());
                    return Response::builder()
                        .status(StatusCode::OK)
                        .header("Content-Type", "application/json")
                        .header("Access-Control-Allow-Origin", "*")
                        .body(Full::new(Bytes::from(body)))
                        .unwrap();
                }
            }
        }
    }

    // Client not found
    let error_response = ErrorResponse {
        error: format!("Client with ID '{client_id}' not found"),
    };
    let body = serde_json::to_string(&error_response).unwrap_or_else(|_| "{}".to_string());
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .header("Content-Type", "application/json")
        .header("Access-Control-Allow-Origin", "*")
        .body(Full::new(Bytes::from(body)))
        .unwrap()
}

// Generate cards for a client
async fn handle_generate_cards(
    req: Request<hyper::body::Incoming>,
    game: &Game,
) -> Response<Full<Bytes>> {
    // Get client ID from headers
    let client_id = match req.headers().get("X-Client-ID") {
        Some(header_value) => {
            match header_value.to_str() {
                Ok(id) => id.to_string(),
                Err(_) => {
                    let error_response = ErrorResponse {
                        error: "Invalid client ID in header".to_string(),
                    };
                    let body = serde_json::to_string(&error_response).unwrap_or_else(|_| "{}".to_string());
                    return Response::builder()
                        .status(StatusCode::BAD_REQUEST)
                        .header("Content-Type", "application/json")
                        .header("Access-Control-Allow-Origin", "*")
                        .body(Full::new(Bytes::from(body)))
                        .unwrap();
                }
            }
        }
        None => {
            let error_response = ErrorResponse {
                error: "Client ID header (X-Client-ID) is required".to_string(),
            };
            let body = serde_json::to_string(&error_response).unwrap_or_else(|_| "{}".to_string());
            return Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .header("Content-Type", "application/json")
                .header("Access-Control-Allow-Origin", "*")
                .body(Full::new(Bytes::from(body)))
                .unwrap();
        }
    };

    // Read the request body
    let body = match req.collect().await {
        Ok(body) => body.to_bytes(),
        Err(_) => {
            let error_response = ErrorResponse {
                error: "Failed to read request body".to_string(),
            };
            let body = serde_json::to_string(&error_response).unwrap_or_else(|_| "{}".to_string());
            return Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .header("Content-Type", "application/json")
                .header("Access-Control-Allow-Origin", "*")
                .body(Full::new(Bytes::from(body)))
                .unwrap();
        }
    };

    // Parse the request
    let generate_request: GenerateCardsRequest = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(_) => {
            let error_response = ErrorResponse {
                error: "Invalid JSON in request body".to_string(),
            };
            let body = serde_json::to_string(&error_response).unwrap_or_else(|_| "{}".to_string());
            return Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .header("Content-Type", "application/json")
                .header("Access-Control-Allow-Origin", "*")
                .body(Full::new(Bytes::from(body)))
                .unwrap();
        }
    };

    // Verify client is registered
    let client_exists = if let Ok(registry) = game.client_registry().lock() {
        registry.values().any(|client| client.id == client_id)
    } else {
        false
    };

    if !client_exists {
        let error_response = ErrorResponse {
            error: "Client not registered".to_string(),
        };
        let body = serde_json::to_string(&error_response).unwrap_or_else(|_| "{}".to_string());
        return Response::builder()
            .status(StatusCode::UNAUTHORIZED)
            .header("Content-Type", "application/json")
            .header("Access-Control-Allow-Origin", "*")
            .body(Full::new(Bytes::from(body)))
            .unwrap();
    }

    // Check if client already has cards assigned (prevent duplicate generation)
    if let Ok(manager) = game.card_manager().lock() {
        if let Some(existing_cards) = manager.get_client_cards(&client_id) {
            if !existing_cards.is_empty() {
                let error_response = ErrorResponse {
                    error: "Client already has cards assigned. Card generation is only allowed during registration.".to_string(),
                };
                let body = serde_json::to_string(&error_response).unwrap_or_else(|_| "{}".to_string());
                return Response::builder()
                    .status(StatusCode::CONFLICT)
                    .header("Content-Type", "application/json")
                    .header("Access-Control-Allow-Origin", "*")
                    .body(Full::new(Bytes::from(body)))
                    .unwrap();
            }
        }
    }

    // Generate cards using the CardAssignmentManager
    let card_infos = if let Ok(mut manager) = game.card_manager().lock() {
        let (cards, _) = manager.assign_cards(client_id.clone(), generate_request.count);
        cards
    } else {
        let error_response = ErrorResponse {
            error: "Failed to acquire card manager lock".to_string(),
        };
        let body = serde_json::to_string(&error_response).unwrap_or_else(|_| "{}".to_string());
        return Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .header("Content-Type", "application/json")
            .header("Access-Control-Allow-Origin", "*")
            .body(Full::new(Bytes::from(body)))
            .unwrap();
    };

    log_info(&format!("Generated {} cards for client {}", card_infos.len(), client_id));

    // Create response
    let response = GenerateCardsResponse {
        cards: card_infos,
        message: format!("Generated {} cards successfully", generate_request.count),
    };

    let body = serde_json::to_string(&response).unwrap_or_else(|_| "{}".to_string());
    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/json")
        .header("Access-Control-Allow-Origin", "*")
        .body(Full::new(Bytes::from(body)))
        .unwrap()
}

// List assigned cards for a client
async fn handle_list_assigned_cards(
    req: Request<hyper::body::Incoming>,
    game: &Game,
) -> Response<Full<Bytes>> {
    // Get client ID from headers
    let client_id = match req.headers().get("X-Client-ID") {
        Some(header_value) => {
            match header_value.to_str() {
                Ok(id) => id.to_string(),
                Err(_) => {
                    let error_response = ErrorResponse {
                        error: "Invalid client ID in header".to_string(),
                    };
                    let body = serde_json::to_string(&error_response).unwrap_or_else(|_| "{}".to_string());
                    return Response::builder()
                        .status(StatusCode::BAD_REQUEST)
                        .header("Content-Type", "application/json")
                        .header("Access-Control-Allow-Origin", "*")
                        .body(Full::new(Bytes::from(body)))
                        .unwrap();
                }
            }
        }
        None => {
            let error_response = ErrorResponse {
                error: "Client ID header (X-Client-ID) is required".to_string(),
            };
            let body = serde_json::to_string(&error_response).unwrap_or_else(|_| "{}".to_string());
            return Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .header("Content-Type", "application/json")
                .header("Access-Control-Allow-Origin", "*")
                .body(Full::new(Bytes::from(body)))
                .unwrap();
        }
    };

    // Verify client is registered
    let client_exists = if let Ok(registry) = game.client_registry().lock() {
        registry.values().any(|client| client.id == client_id)
    } else {
        false
    };

    if !client_exists {
        let error_response = ErrorResponse {
            error: "Client not registered".to_string(),
        };
        let body = serde_json::to_string(&error_response).unwrap_or_else(|_| "{}".to_string());
        return Response::builder()
            .status(StatusCode::UNAUTHORIZED)
            .header("Content-Type", "application/json")
            .header("Access-Control-Allow-Origin", "*")
            .body(Full::new(Bytes::from(body)))
            .unwrap();
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
            card_id: card_id.clone(),
            assigned_to: client_id.clone(),
        }
    }).collect();

    let response = ListAssignedCardsResponse {
        cards: card_infos,
    };

    let body = serde_json::to_string(&response).unwrap_or_else(|_| "{}".to_string());
    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/json")
        .header("Access-Control-Allow-Origin", "*")
        .body(Full::new(Bytes::from(body)))
        .unwrap()
}

// Get a specific assigned card
async fn handle_get_assigned_card(
    req: Request<hyper::body::Incoming>,
    game: &Game,
    card_id: String,
) -> Response<Full<Bytes>> {
    // Get client ID from headers
    let client_id = match req.headers().get("X-Client-ID") {
        Some(header_value) => {
            match header_value.to_str() {
                Ok(id) => id.to_string(),
                Err(_) => {
                    let error_response = ErrorResponse {
                        error: "Invalid client ID in header".to_string(),
                    };
                    let body = serde_json::to_string(&error_response).unwrap_or_else(|_| "{}".to_string());
                    return Response::builder()
                        .status(StatusCode::BAD_REQUEST)
                        .header("Content-Type", "application/json")
                        .header("Access-Control-Allow-Origin", "*")
                        .body(Full::new(Bytes::from(body)))
                        .unwrap();
                }
            }
        }
        None => {
            let error_response = ErrorResponse {
                error: "Client ID header (X-Client-ID) is required".to_string(),
            };
            let body = serde_json::to_string(&error_response).unwrap_or_else(|_| "{}".to_string());
            return Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .header("Content-Type", "application/json")
                .header("Access-Control-Allow-Origin", "*")
                .body(Full::new(Bytes::from(body)))
                .unwrap();
        }
    };

    // Verify client is registered
    let client_exists = if let Ok(registry) = game.client_registry().lock() {
        registry.values().any(|client| client.id == client_id)
    } else {
        false
    };

    if !client_exists {
        let error_response = ErrorResponse {
            error: "Client not registered".to_string(),
        };
        let body = serde_json::to_string(&error_response).unwrap_or_else(|_| "{}".to_string());
        return Response::builder()
            .status(StatusCode::UNAUTHORIZED)
            .header("Content-Type", "application/json")
            .header("Access-Control-Allow-Origin", "*")
            .body(Full::new(Bytes::from(body)))
            .unwrap();
    }

    // Get the card assignment
    let card_assignment = if let Ok(manager) = game.card_manager().lock() {
        manager.get_card_assignment(&card_id).cloned()
    } else {
        None
    };

    // Verify the card exists and belongs to the client
    let card_assignment = match card_assignment {
        Some(assignment) => {
            if assignment.client_id != client_id {
                let error_response = ErrorResponse {
                    error: "Card not assigned to this client".to_string(),
                };
                let body = serde_json::to_string(&error_response).unwrap_or_else(|_| "{}".to_string());
                return Response::builder()
                    .status(StatusCode::FORBIDDEN)
                    .header("Content-Type", "application/json")
                    .header("Access-Control-Allow-Origin", "*")
                    .body(Full::new(Bytes::from(body)))
                    .unwrap();
            }
            assignment
        }
        None => {
            let error_response = ErrorResponse {
                error: "Card not found".to_string(),
            };
            let body = serde_json::to_string(&error_response).unwrap_or_else(|_| "{}".to_string());
            return Response::builder()
                .status(StatusCode::NOT_FOUND)
                .header("Content-Type", "application/json")
                .header("Access-Control-Allow-Origin", "*")
                .body(Full::new(Bytes::from(body)))
                .unwrap();
        }
    };

    // Create response
    let card_info = CardInfo {
        card_id: card_assignment.card_id,
        card_data: card_assignment.card_data.iter().map(|row| {
            row.to_vec()
        }).collect(),
    };

    let body = serde_json::to_string(&card_info).unwrap_or_else(|_| "{}".to_string());
    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/json")
        .header("Access-Control-Allow-Origin", "*")
        .body(Full::new(Bytes::from(body)))
        .unwrap()
}

// Handle board endpoint
async fn handle_board(game: &Game) -> Response<Full<Bytes>> {
    let body = if let Ok(board) = game.board().lock() {
        serde_json::to_string(&*board).unwrap_or_else(|_| "{}".to_string())
    } else {
        serde_json::to_string(&Board::new()).unwrap_or_else(|_| "{}".to_string())
    };

    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/json")
        .header("Access-Control-Allow-Origin", "*")
        .body(Full::new(Bytes::from(body)))
        .unwrap()
}

// Handle pouch endpoint
async fn handle_pouch(game: &Game) -> Response<Full<Bytes>> {
    let body = if let Ok(pouch) = game.pouch().lock() {
        serde_json::to_string(&*pouch).unwrap_or_else(|_| "{}".to_string())
    } else {
        serde_json::to_string(&Pouch::new()).unwrap_or_else(|_| "{}".to_string())
    };

    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/json")
        .header("Access-Control-Allow-Origin", "*")
        .body(Full::new(Bytes::from(body)))
        .unwrap()
}

// Handle scoremap endpoint
async fn handle_scoremap(game: &Game) -> Response<Full<Bytes>> {
    let body = if let Ok(scorecard) = game.scorecard().lock() {
        serde_json::to_string(&*scorecard).unwrap_or_else(|_| "{}".to_string())
    } else {
        serde_json::to_string(&ScoreCard::new()).unwrap_or_else(|_| "{}".to_string())
    };

    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/json")
        .header("Access-Control-Allow-Origin", "*")
        .body(Full::new(Bytes::from(body)))
        .unwrap()
}

// Handle status endpoint
async fn handle_status(game: &Game) -> Response<Full<Bytes>> {
    let board_len = game.board_length();
    let scorecard = game.published_score();
    let response = json!({
        "status": "running",
        "game_id": game.id(),
        "created_at": game.created_at_string(),
        "numbers_extracted": board_len,
        "scorecard": scorecard,
        "server": "tokio-hyper"
    });
    let body = serde_json::to_string(&response).unwrap_or_else(|_| "{}".to_string());
    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/json")
        .header("Access-Control-Allow-Origin", "*")
        .body(Full::new(Bytes::from(body)))
        .unwrap()
}

// Handle running game ID endpoint
async fn handle_running_game_id(game: &Game) -> Response<Full<Bytes>> {
    let (game_id, created_at_string, created_at_systemtime) = game.get_running_game_info();
    let response = json!({
        "game_id": game_id,
        "created_at": created_at_string,
        "created_at_timestamp": {
            "secs_since_epoch": created_at_systemtime.duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default().as_secs(),
            "nanos_since_epoch": created_at_systemtime.duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default().subsec_nanos()
        }
    });
    let body = serde_json::to_string(&response).unwrap_or_else(|_| "{}".to_string());
    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/json")
        .header("Access-Control-Allow-Origin", "*")
        .body(Full::new(Bytes::from(body)))
        .unwrap()
}

// Handle extract endpoint - performs number extraction
async fn handle_extract(
    req: Request<hyper::body::Incoming>,
    game: &Game,
) -> Response<Full<Bytes>> {
    // Get client ID from headers for authentication
    let client_id = match req.headers().get("X-Client-ID") {
        Some(header_value) => {
            match header_value.to_str() {
                Ok(id) => id.to_string(),
                Err(_) => {
                    let error_response = ErrorResponse {
                        error: "Invalid client ID in header".to_string(),
                    };
                    let body = serde_json::to_string(&error_response).unwrap_or_else(|_| "{}".to_string());
                    return Response::builder()
                        .status(StatusCode::BAD_REQUEST)
                        .header("Content-Type", "application/json")
                        .header("Access-Control-Allow-Origin", "*")
                        .body(Full::new(Bytes::from(body)))
                        .unwrap();
                }
            }
        }
        None => {
            let error_response = ErrorResponse {
                error: "Client ID header (X-Client-ID) is required".to_string(),
            };
            let body = serde_json::to_string(&error_response).unwrap_or_else(|_| "{}".to_string());
            return Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .header("Content-Type", "application/json")
                .header("Access-Control-Allow-Origin", "*")
                .body(Full::new(Bytes::from(body)))
                .unwrap();
        }
    };

    // Only allow board client (ID: "0000000000000000") to extract numbers
    if client_id != BOARD_ID {
        let error_response = ErrorResponse {
            error: "Unauthorized: Only board client can extract numbers".to_string(),
        };
        let body = serde_json::to_string(&error_response).unwrap_or_else(|_| "{}".to_string());
        return Response::builder()
            .status(StatusCode::FORBIDDEN)
            .header("Content-Type", "application/json")
            .header("Access-Control-Allow-Origin", "*")
            .body(Full::new(Bytes::from(body)))
            .unwrap();
    }

    // Check if BINGO has been reached - if so, no more extractions allowed
    if game.is_bingo_reached() {
        let error_response = ErrorResponse {
            error: "Game over: BINGO has been reached. No more numbers can be extracted.".to_string(),
        };
        let body = serde_json::to_string(&error_response).unwrap_or_else(|_| "{}".to_string());
        return Response::builder()
            .status(StatusCode::CONFLICT)
            .header("Content-Type", "application/json")
            .header("Access-Control-Allow-Origin", "*")
            .body(Full::new(Bytes::from(body)))
            .unwrap();
    }

    // Extract a number using the game's coordinated extraction logic
    match game.extract_number(0) {
        Ok((extracted_number, _new_working_score)) => {
            // Get current pouch and board state for response using Game methods
            let numbers_remaining = game.pouch_length();
            let total_extracted = game.board_length();

            // Check if BINGO was reached after this extraction and dump game state if so
            if game.is_bingo_reached() {
                match game.dump_to_json() {
                    Ok(dump_message) => {
                        crate::logging::log_info(&format!("Game ended with BINGO! {}", dump_message));
                    }
                    Err(dump_error) => {
                        crate::logging::log_error(&format!("Failed to dump game state: {}", dump_error));
                    }
                }
            }

            // Create success response
            let response = json!({
                "success": true,
                "extracted_number": extracted_number,
                "numbers_remaining": numbers_remaining,
                "total_extracted": total_extracted,
                "message": format!("Number {} extracted successfully", extracted_number)
            });

            let body = serde_json::to_string(&response).unwrap_or_else(|_| "{}".to_string());
            Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "application/json")
                .header("Access-Control-Allow-Origin", "*")
                .body(Full::new(Bytes::from(body)))
                .unwrap()
        }
        Err(error_msg) => {
            // Handle extraction errors
            let status_code = if error_msg.contains("empty") {
                StatusCode::CONFLICT
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            };

            let error_response = ErrorResponse {
                error: error_msg,
            };
            let body = serde_json::to_string(&error_response).unwrap_or_else(|_| "{}".to_string());
            Response::builder()
                .status(status_code)
                .header("Content-Type", "application/json")
                .header("Access-Control-Allow-Origin", "*")
                .body(Full::new(Bytes::from(body)))
                .unwrap()
        }
    }
}

// Handle newgame endpoint - resets all game state
async fn handle_newgame(
    req: Request<hyper::body::Incoming>,
    game: &Game,
) -> Response<Full<Bytes>> {
    // Get client ID from headers for authentication
    let client_id = match req.headers().get("X-Client-ID") {
        Some(header_value) => {
            match header_value.to_str() {
                Ok(id) => id.to_string(),
                Err(_) => {
                    let error_response = ErrorResponse {
                        error: "Invalid client ID in header".to_string(),
                    };
                    let body = serde_json::to_string(&error_response).unwrap_or_else(|_| "{}".to_string());
                    return Response::builder()
                        .status(StatusCode::BAD_REQUEST)
                        .header("Content-Type", "application/json")
                        .header("Access-Control-Allow-Origin", "*")
                        .body(Full::new(Bytes::from(body)))
                        .unwrap();
                }
            }
        }
        None => {
            let error_response = ErrorResponse {
                error: "Client ID header (X-Client-ID) is required".to_string(),
            };
            let body = serde_json::to_string(&error_response).unwrap_or_else(|_| "{}".to_string());
            return Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .header("Content-Type", "application/json")
                .header("Access-Control-Allow-Origin", "*")
                .body(Full::new(Bytes::from(body)))
                .unwrap();
        }
    };

    // Only allow board client (ID: "0000000000000000") to reset the game
    if client_id != BOARD_ID {
        let error_response = ErrorResponse {
            error: "Unauthorized: Only board client can reset the game".to_string(),
        };
        let body = serde_json::to_string(&error_response).unwrap_or_else(|_| "{}".to_string());
        return Response::builder()
            .status(StatusCode::FORBIDDEN)
            .header("Content-Type", "application/json")
            .header("Access-Control-Allow-Origin", "*")
            .body(Full::new(Bytes::from(body)))
            .unwrap();
    }

    // Dump the current game state only if the game has started but BINGO was not reached
    // (BINGO games are already auto-dumped when BINGO occurs)
    if game.has_game_started() && !game.is_bingo_reached() {
        match game.dump_to_json() {
            Ok(dump_message) => {
                log_info(&format!("Incomplete game dumped before reset: {}", dump_message));
            }
            Err(dump_error) => {
                log_error(&format!("Failed to dump incomplete game state before reset: {}", dump_error));
            }
        }
    }

    // Use the Game struct's reset_game method which handles proper mutex coordination
    match game.reset_game() {
        Ok(reset_components) => {
            log_info(&format!("Game reset successful for {}", game.game_info()));
            let response = json!({
                "success": true,
                "message": "New game started successfully",
                "reset_components": reset_components,
                "game_id": game.id(),
                "created_at": game.created_at_string()
            });
            let body = serde_json::to_string(&response).unwrap_or_else(|_| "{}".to_string());
            Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "application/json")
                .header("Access-Control-Allow-Origin", "*")
                .body(Full::new(Bytes::from(body)))
                .unwrap()
        }
        Err(errors) => {
            log_error_stderr(&format!("Game reset failed: {:?}", errors));
            let error_response = ErrorResponse {
                error: format!("Failed to reset game: {}", errors.join(", ")),
            };
            let body = serde_json::to_string(&error_response).unwrap_or_else(|_| "{}".to_string());
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .header("Content-Type", "application/json")
                .header("Access-Control-Allow-Origin", "*")
                .body(Full::new(Bytes::from(body)))
                .unwrap()
        }
    }
}

// Handle dumpgame endpoint - dumps current game state to JSON file
async fn handle_dumpgame(
    req: Request<hyper::body::Incoming>,
    game: &Game,
) -> Response<Full<Bytes>> {
    // Check for client authentication header
    let client_id = match req.headers().get("X-Client-ID") {
        Some(header_value) => {
            match header_value.to_str() {
                Ok(id) => id,
                Err(_) => {
                    let error_response = ErrorResponse {
                        error: "Invalid X-Client-ID header".to_string(),
                    };
                    let body = serde_json::to_string(&error_response).unwrap_or_else(|_| "{}".to_string());
                    return Response::builder()
                        .status(StatusCode::BAD_REQUEST)
                        .header("Content-Type", "application/json")
                        .header("Access-Control-Allow-Origin", "*")
                        .body(Full::new(Bytes::from(body)))
                        .unwrap();
                }
            }
        }
        None => {
            let error_response = ErrorResponse {
                error: "Missing X-Client-ID header".to_string(),
            };
            let body = serde_json::to_string(&error_response).unwrap_or_else(|_| "{}".to_string());
            return Response::builder()
                .status(StatusCode::UNAUTHORIZED)
                .header("Content-Type", "application/json")
                .header("Access-Control-Allow-Origin", "*")
                .body(Full::new(Bytes::from(body)))
                .unwrap();
        }
    };

    // Only allow board client (ID: "0000000000000000") to dump the game
    if client_id != BOARD_ID {
        let error_response = ErrorResponse {
            error: "Unauthorized: Only board client can dump the game".to_string(),
        };
        let body = serde_json::to_string(&error_response).unwrap_or_else(|_| "{}".to_string());
        return Response::builder()
            .status(StatusCode::FORBIDDEN)
            .header("Content-Type", "application/json")
            .header("Access-Control-Allow-Origin", "*")
            .body(Full::new(Bytes::from(body)))
            .unwrap();
    }

    // Dump the game state to JSON
    match game.dump_to_json() {
        Ok(dump_message) => {
            log_info(&format!("Game manually dumped: {}", dump_message));
            let response = json!({
                "success": true,
                "message": dump_message,
                "game_id": game.id(),
                "game_ended": game.is_game_ended(),
                "bingo_reached": game.is_bingo_reached(),
                "pouch_empty": game.is_pouch_empty()
            });
            let body = serde_json::to_string(&response).unwrap_or_else(|_| "{}".to_string());
            Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "application/json")
                .header("Access-Control-Allow-Origin", "*")
                .body(Full::new(Bytes::from(body)))
                .unwrap()
        }
        Err(dump_error) => {
            log_error(&format!("Manual game dump failed: {}", dump_error));
            let error_response = ErrorResponse {
                error: format!("Failed to dump game: {}", dump_error),
            };
            let body = serde_json::to_string(&error_response).unwrap_or_else(|_| "{}".to_string());
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .header("Content-Type", "application/json")
                .header("Access-Control-Allow-Origin", "*")
                .body(Full::new(Bytes::from(body)))
                .unwrap()
        }
    }
}

// Handle 404 not found
async fn handle_not_found() -> Response<Full<Bytes>> {
    let response = ErrorResponse {
        error: "Not found".to_string(),
    };
    let body = serde_json::to_string(&response).unwrap_or_else(|_| "{}".to_string());
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .header("Content-Type", "application/json")
        .header("Access-Control-Allow-Origin", "*")
        .body(Full::new(Bytes::from(body)))
        .unwrap()
}

// Generate client ID based on name and type
