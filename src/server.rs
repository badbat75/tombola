use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::convert::Infallible;
use std::net::SocketAddr;
use std::collections::HashMap;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{body::Bytes, Request, Response, StatusCode, Method};
use hyper_util::rt::TokioIo;
use tokio::net::TcpListener;
use http_body_util::{Full, BodyExt};
use serde_json::json;

// Import Board from board module
use crate::board::Board;
use crate::pouch::Pouch;
use crate::score::ScoreCard;
use crate::defs::Number;
use crate::client::{RegisterRequest, RegisterResponse, ClientInfoResponse, ClientInfo, ClientRegistry};
use crate::card::{CardAssignmentManager, GenerateCardsRequest, GenerateCardsResponse, CardInfo, ListAssignedCardsResponse, AssignedCardInfo};
use crate::config::ServerConfig;

// Import the extraction function from extraction module
use crate::extraction::perform_extraction;

// Response structures for JSON serialization
#[derive(serde::Serialize)]
struct ErrorResponse {
    error: String,
}

// Start the HTTP server with Tokio
pub fn start_server(board_ref: Arc<Mutex<Board>>, pouch_ref: Arc<Mutex<Pouch>>, scorecard_ref: Arc<Mutex<ScoreCard>>, config: ServerConfig) -> (tokio::task::JoinHandle<()>, Arc<AtomicBool>, Arc<Mutex<CardAssignmentManager>>) {
    let shutdown_signal = Arc::new(AtomicBool::new(false));
    let shutdown_clone = Arc::clone(&shutdown_signal);
    let client_registry: ClientRegistry = Arc::new(Mutex::new(HashMap::new()));
    let card_manager: Arc<Mutex<CardAssignmentManager>> = Arc::new(Mutex::new(CardAssignmentManager::new()));
    
    let card_manager_clone = Arc::clone(&card_manager);
    
    let handle = tokio::spawn(async move {
        let addr = SocketAddr::from((config.host.parse::<std::net::IpAddr>().unwrap_or([127, 0, 0, 1].into()), config.port));
        let listener = match TcpListener::bind(&addr).await {
            Ok(listener) => listener,
            Err(e) => {
                eprintln!("Failed to start API server: {e}");
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
                    let board_clone = Arc::clone(&board_ref);
                    let pouch_clone = Arc::clone(&pouch_ref);
                    let scorecard_clone = Arc::clone(&scorecard_ref);
                    let registry_clone = Arc::clone(&client_registry);
                    let card_manager_clone = Arc::clone(&card_manager);
                    let io = TokioIo::new(stream);
                    
                    // Spawn a task to handle the connection
                    tokio::spawn(async move {
                        let service = service_fn(move |req| {
                            handle_request(req, Arc::clone(&board_clone), Arc::clone(&pouch_clone), Arc::clone(&scorecard_clone), registry_clone.clone(), Arc::clone(&card_manager_clone))
                        });
                        
                        if let Err(err) = http1::Builder::new()
                            .serve_connection(io, service)
                            .await
                        {
                            eprintln!("Error serving connection: {err:?}");
                        }
                    });
                }
                Ok(Err(e)) => {
                    eprintln!("Error accepting connection: {e}");
                    break;
                }
                Err(_) => {
                    // Timeout occurred, continue to check shutdown signal
                }
            }
        }
        println!("API Server shutting down...");
    });

    (handle, shutdown_signal, card_manager_clone)
}

// Handle HTTP requests asynchronously
async fn handle_request(
    req: Request<hyper::body::Incoming>,
    board_ref: Arc<Mutex<Board>>,
    pouch_ref: Arc<Mutex<Pouch>>,
    scorecard_ref: Arc<Mutex<ScoreCard>>,
    client_registry: ClientRegistry,
    card_manager: Arc<Mutex<CardAssignmentManager>>,
) -> Result<Response<Full<Bytes>>, Infallible> {
    let response = match (req.method(), req.uri().path()) {
        (&Method::POST, "/register") => {
            handle_register(req, client_registry, card_manager).await
        }
        (&Method::GET, path) if path.starts_with("/client/") => {
            let client_name = &path[8..]; // Remove "/client/" prefix
            handle_client_info(client_name, client_registry).await
        }
        (&Method::GET, path) if path.starts_with("/clientbyid/") => {
            let client_id = &path[12..]; // Remove "/clientbyid/" prefix
            handle_client_info_by_id(client_id, client_registry).await
        }
        (&Method::POST, "/generatecardsforme") => {
            handle_generate_cards(req, client_registry, card_manager).await
        }
        (&Method::GET, "/listassignedcards") => {
            handle_list_assigned_cards(req, client_registry, card_manager).await
        }
        (&Method::GET, path) if path.starts_with("/getassignedcard/") => {
            let card_id = path[17..].to_string(); // Remove "/getassignedcard/" prefix
            handle_get_assigned_card(req, client_registry, card_manager, card_id).await
        }
        (&Method::GET, "/board") => {
            handle_board(board_ref).await
        }
        (&Method::GET, "/pouch") => {
            handle_pouch(pouch_ref).await
        }
        (&Method::GET, "/scoremap") => {
            handle_scoremap(scorecard_ref).await
        }
        (&Method::POST, "/extract") => {
            handle_extract(req, board_ref, pouch_ref, scorecard_ref, card_manager, client_registry).await
        }
        (&Method::POST, "/newgame") => {
            handle_newgame(req, board_ref, pouch_ref, scorecard_ref, card_manager).await
        }
        (&Method::GET, "/status") => {
            handle_status(board_ref, scorecard_ref).await
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
    client_registry: ClientRegistry,
    card_manager: Arc<Mutex<CardAssignmentManager>>,
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
        register_request.name.clone(),
        register_request.client_type.clone(),
    );
    let client_id = client_info.id.clone();

    // Check if client already exists and return existing info
    if let Ok(mut registry) = client_registry.lock() {
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

        // Store new client information (using name as key)
        registry.insert(register_request.name.clone(), client_info);
        println!("✅ Client registered: {} (ID: {})", register_request.name, client_id);
    }

    // Check if client requested cards during registration, default to 1 if not specified
    let card_count = register_request.nocard.unwrap_or(1);
    println!("🎴 Generating {} cards for client '{}' during registration", card_count, register_request.name);
    
    // Generate the requested number of cards using the card manager
    if let Ok(mut manager) = card_manager.lock() {
        manager.assign_cards(client_id.clone(), card_count);
        println!("✅ Generated and assigned {} cards to client '{}'", card_count, register_request.name);
    } else {
        println!("⚠️  Failed to acquire card manager lock for client '{}'", register_request.name);
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



// Function to get the board length
fn get_board_length(board_ref: &Arc<Mutex<Board>>) -> usize {
    if let Ok(board) = board_ref.lock() {
        board.len()
    } else {
        0
    }
}

// Function to get the scorecard value from the scorecard
fn get_scorecard_from_scorecard(scorecard_ref: &Arc<Mutex<ScoreCard>>) -> Number {
    if let Ok(scorecard) = scorecard_ref.lock() {
        scorecard.published_score
    } else {
        0
    }
}

// Function to get pouch information
async fn handle_client_info(
    client_name: &str,
    client_registry: ClientRegistry,
) -> Response<Full<Bytes>> {
    // Look up client by name
    if let Ok(registry) = client_registry.lock() {
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
    client_registry: ClientRegistry,
) -> Response<Full<Bytes>> {
    // Use ClientInfo method to resolve client name (handles both special board case and regular clients)
    let client_name = ClientInfo::get_client_name_by_id(client_id, &client_registry)
        .unwrap_or_else(|| "Unknown".to_string());

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
        if let Ok(registry) = client_registry.lock() {
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
    client_registry: ClientRegistry,
    card_manager: Arc<Mutex<CardAssignmentManager>>,
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
    let client_exists = if let Ok(registry) = client_registry.lock() {
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
    if let Ok(manager) = card_manager.lock() {
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
    let card_infos = if let Ok(mut manager) = card_manager.lock() {
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

    println!("✅ Generated {} cards for client {}", card_infos.len(), client_id);

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
    client_registry: ClientRegistry,
    card_manager: Arc<Mutex<CardAssignmentManager>>,
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
    let client_exists = if let Ok(registry) = client_registry.lock() {
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
    let assigned_cards = if let Ok(manager) = card_manager.lock() {
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
    client_registry: ClientRegistry,
    card_manager: Arc<Mutex<CardAssignmentManager>>,
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
    let client_exists = if let Ok(registry) = client_registry.lock() {
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
    let card_assignment = if let Ok(manager) = card_manager.lock() {
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
async fn handle_board(board_ref: Arc<Mutex<Board>>) -> Response<Full<Bytes>> {
    let board = if let Ok(board) = board_ref.lock() {
        board.clone()
    } else {
        Board::new()
    };
    
    let body = serde_json::to_string(&board).unwrap_or_else(|_| "{}".to_string());
    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/json")
        .header("Access-Control-Allow-Origin", "*")
        .body(Full::new(Bytes::from(body)))
        .unwrap()
}

// Handle pouch endpoint
async fn handle_pouch(pouch_ref: Arc<Mutex<Pouch>>) -> Response<Full<Bytes>> {
    let pouch = if let Ok(pouch) = pouch_ref.lock() {
        pouch.clone()
    } else {
        Pouch::new()
    };
    let body = serde_json::to_string(&pouch).unwrap_or_else(|_| "{}".to_string());
    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/json")
        .header("Access-Control-Allow-Origin", "*")
        .body(Full::new(Bytes::from(body)))
        .unwrap()
}

// Handle scoremap endpoint
async fn handle_scoremap(scorecard_ref: Arc<Mutex<ScoreCard>>) -> Response<Full<Bytes>> {
    let scorecard = if let Ok(scorecard) = scorecard_ref.lock() {
        scorecard.clone()
    } else {
        ScoreCard::new()
    };
    
    let body = serde_json::to_string(&scorecard).unwrap_or_else(|_| "{}".to_string());
    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/json")
        .header("Access-Control-Allow-Origin", "*")
        .body(Full::new(Bytes::from(body)))
        .unwrap()
}

// Handle status endpoint
async fn handle_status(board_ref: Arc<Mutex<Board>>, scorecard_ref: Arc<Mutex<ScoreCard>>) -> Response<Full<Bytes>> {
    let board_len = get_board_length(&board_ref);
    let scorecard = get_scorecard_from_scorecard(&scorecard_ref);
    let response = json!({
        "status": "running",
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

// Handle extract endpoint - performs number extraction
async fn handle_extract(
    req: Request<hyper::body::Incoming>,
    board_ref: Arc<Mutex<Board>>,
    pouch_ref: Arc<Mutex<Pouch>>,
    scorecard_ref: Arc<Mutex<ScoreCard>>,
    card_manager: Arc<Mutex<CardAssignmentManager>>,
    #[allow(unused_variables)] registry: ClientRegistry,
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
    if client_id != "0000000000000000" {
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
    if let Ok(scorecard) = scorecard_ref.lock() {
        if scorecard.published_score >= 15 { // BINGO reached
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
    }

    // Extract a number using the shared extraction logic
    match perform_extraction(&pouch_ref, &board_ref, &scorecard_ref, &card_manager, 0) {
        Ok((extracted_number, _new_working_score)) => {
            // Get current pouch and board state for response
            let numbers_remaining = if let Ok(pouch) = pouch_ref.lock() {
                pouch.len()
            } else {
                0
            };
            
            let total_extracted = if let Ok(board) = board_ref.lock() {
                board.len()
            } else {
                0
            };
            
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
    board_ref: Arc<Mutex<Board>>,
    pouch_ref: Arc<Mutex<Pouch>>,
    scorecard_ref: Arc<Mutex<ScoreCard>>,
    card_manager: Arc<Mutex<CardAssignmentManager>>,
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
    if client_id != "0000000000000000" {
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

    // Reset all game structures in coordinated order to prevent deadlocks
    // Follow the mutex acquisition order: pouch -> board -> scorecard -> card_manager
    let mut reset_components = Vec::new();
    let mut errors = Vec::new();

    // Reset Pouch (refill with numbers 1-90)
    if let Ok(mut pouch) = pouch_ref.lock() {
        *pouch = Pouch::new();
        reset_components.push("Pouch refilled with numbers 1-90".to_string());
    } else {
        errors.push("Failed to lock pouch for reset".to_string());
    }

    // Reset Board (clear extracted numbers and marked positions)
    if let Ok(mut board) = board_ref.lock() {
        *board = Board::new();
        reset_components.push("Board state cleared".to_string());
    } else {
        errors.push("Failed to lock board for reset".to_string());
    }

    // Reset ScoreCard (reset published score and score map)
    if let Ok(mut scorecard) = scorecard_ref.lock() {
        *scorecard = ScoreCard::new();
        reset_components.push("Score card reset".to_string());
    } else {
        errors.push("Failed to lock scorecard for reset".to_string());
    }

    // Reset CardAssignmentManager (clear all card assignments)
    if let Ok(mut card_mgr) = card_manager.lock() {
        *card_mgr = CardAssignmentManager::new();
        reset_components.push("Card assignments cleared".to_string());
    } else {
        errors.push("Failed to lock card manager for reset".to_string());
    }

    // Check if any errors occurred during reset
    if !errors.is_empty() {
        let error_response = ErrorResponse {
            error: format!("Game reset partially failed: {}", errors.join(", ")),
        };
        let body = serde_json::to_string(&error_response).unwrap_or_else(|_| "{}".to_string());
        return Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .header("Content-Type", "application/json")
            .header("Access-Control-Allow-Origin", "*")
            .body(Full::new(Bytes::from(body)))
            .unwrap();
    }

    println!("🔄 Game reset initiated via API by client {}", client_id);

    // Create success response
    let response = json!({
        "success": true,
        "message": "New game started successfully",
        "reset_components": reset_components
    });

    let body = serde_json::to_string(&response).unwrap_or_else(|_| "{}".to_string());
    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/json")
        .header("Access-Control-Allow-Origin", "*")
        .body(Full::new(Bytes::from(body)))
        .unwrap()
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
