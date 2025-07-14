use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::convert::Infallible;
use std::net::SocketAddr;
use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::Hasher;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{body::Bytes, Request, Response, StatusCode, Method};
use hyper_util::rt::TokioIo;
use tokio::net::TcpListener;
use http_body_util::{Full, BodyExt};
use serde_json::json;
use serde::{Deserialize, Serialize};

// Import Board from board module
use crate::board::Board;
use crate::pouch::Pouch;
use crate::defs::Number;
use crate::card::{TombolaGenerator, Card};

// Response structures for JSON serialization
#[derive(serde::Serialize)]
struct BoardResponse {
    board: Vec<Number>,
}

#[derive(serde::Serialize)]
struct ScorecardResponse {
    scorecard: Number,
}

#[derive(serde::Serialize)]
struct PouchResponse {
    pouch: Vec<Number>,
    remaining: usize,
}

#[derive(serde::Serialize)]
struct ErrorResponse {
    error: String,
}

// Client registration structures
#[derive(Debug, Deserialize)]
struct RegisterRequest {
    name: String,
    client_type: String,
    nocard: Option<u32>,  // Number of cards to generate during registration
}

#[derive(Debug, Serialize)]
struct RegisterResponse {
    client_id: String,
    message: String,
}

#[derive(Debug, Serialize)]
struct ClientInfoResponse {
    client_id: String,
    name: String,
    client_type: String,
    registered_at: String,
}

// Card generation request
#[derive(Debug, Deserialize)]
struct GenerateCardsRequest {
    count: u32,
}

// Card generation response
#[derive(Debug, Serialize)]
struct GenerateCardsResponse {
    cards: Vec<CardInfo>,
    message: String,
}

// Card info for responses
#[derive(Debug, Serialize)]
struct CardInfo {
    card_id: String,
    card_data: Vec<Vec<Option<u8>>>, // Changed to Option<u8> to match Card structure
}

// List assigned cards response
#[derive(Debug, Serialize)]
struct ListAssignedCardsResponse {
    cards: Vec<AssignedCardInfo>,
}

// Assigned card info
#[derive(Debug, Serialize)]
struct AssignedCardInfo {
    card_id: String,
    assigned_to: String,
}

// Client information storage
#[derive(Debug, Clone)]
struct ClientInfo {
    id: String,
    name: String,
    client_type: String,
    registered_at: std::time::SystemTime,
}

// Card assignment storage
#[derive(Debug, Clone)]
struct CardAssignment {
    card_id: String,
    client_id: String,
    card_data: Card,
}

// Global client registry (keyed by client name)
type ClientRegistry = Arc<Mutex<HashMap<String, ClientInfo>>>;

// Global card assignments registry (keyed by card_id)
type CardAssignments = Arc<Mutex<HashMap<String, CardAssignment>>>;

// Client cards registry (keyed by client_id, contains list of card_ids)
type ClientCards = Arc<Mutex<HashMap<String, Vec<String>>>>;

// Start the HTTP server with Tokio
pub fn start_server(board_ref: Arc<Mutex<Board>>, pouch_ref: Arc<Mutex<Pouch>>) -> (tokio::task::JoinHandle<()>, Arc<AtomicBool>) {
    let shutdown_signal = Arc::new(AtomicBool::new(false));
    let shutdown_clone = Arc::clone(&shutdown_signal);
    let client_registry: ClientRegistry = Arc::new(Mutex::new(HashMap::new()));
    let card_assignments: CardAssignments = Arc::new(Mutex::new(HashMap::new()));
    let client_cards: ClientCards = Arc::new(Mutex::new(HashMap::new()));
    
    let handle = tokio::spawn(async move {
        let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
        let listener = match TcpListener::bind(&addr).await {
            Ok(listener) => {
                println!("API Server started on http://127.0.0.1:3000");
                listener
            }
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
                    let registry_clone = Arc::clone(&client_registry);
                    let cards_clone = Arc::clone(&card_assignments);
                    let client_cards_clone = Arc::clone(&client_cards);
                    let io = TokioIo::new(stream);
                    
                    // Spawn a task to handle the connection
                    tokio::spawn(async move {
                        let service = service_fn(move |req| {
                            handle_request(req, Arc::clone(&board_clone), Arc::clone(&pouch_clone), Arc::clone(&registry_clone), Arc::clone(&cards_clone), Arc::clone(&client_cards_clone))
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

    (handle, shutdown_signal)
}

// Handle HTTP requests asynchronously
async fn handle_request(
    req: Request<hyper::body::Incoming>,
    board_ref: Arc<Mutex<Board>>,
    pouch_ref: Arc<Mutex<Pouch>>,
    client_registry: ClientRegistry,
    card_assignments: CardAssignments,
    client_cards: ClientCards,
) -> Result<Response<Full<Bytes>>, Infallible> {
    let response = match (req.method(), req.uri().path()) {
        (&Method::POST, "/register") => {
            handle_register(req, client_registry, card_assignments, client_cards).await
        }
        (&Method::GET, path) if path.starts_with("/client/") => {
            let client_name = &path[8..]; // Remove "/client/" prefix
            handle_client_info(client_name, client_registry).await
        }
        (&Method::POST, "/generatecardsforme") => {
            handle_generate_cards(req, client_registry, card_assignments, client_cards).await
        }
        (&Method::GET, "/listassignedcards") => {
            handle_list_assigned_cards(req, client_registry, client_cards).await
        }
        (&Method::GET, path) if path.starts_with("/getassignedcard/") => {
            let card_id = path[17..].to_string(); // Remove "/getassignedcard/" prefix
            handle_get_assigned_card(req, client_registry, card_assignments, card_id).await
        }
        (&Method::GET, "/board") => {
            let numbers = get_numbers_from_board(&board_ref);
            let response = BoardResponse { board: numbers };
            let body = serde_json::to_string(&response).unwrap_or_else(|_| "{}".to_string());
            Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "application/json")
                .header("Access-Control-Allow-Origin", "*")
                .body(Full::new(Bytes::from(body)))
                .unwrap()
        }
        (&Method::GET, "/pouch") => {
            let (pouch_numbers, remaining) = get_pouch_info(&pouch_ref);
            let response = PouchResponse { pouch: pouch_numbers, remaining };
            let body = serde_json::to_string(&response).unwrap_or_else(|_| "{}".to_string());
            Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "application/json")
                .header("Access-Control-Allow-Origin", "*")
                .body(Full::new(Bytes::from(body)))
                .unwrap()
        }
        (&Method::GET, "/scorecard") => {
            let scorecard = get_scorecard_from_board(&board_ref);
            let response = ScorecardResponse { scorecard };
            let body = serde_json::to_string(&response).unwrap_or_else(|_| "{}".to_string());
            Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "application/json")
                .header("Access-Control-Allow-Origin", "*")
                .body(Full::new(Bytes::from(body)))
                .unwrap()
        }
        (&Method::GET, "/status") => {
            let board_len = get_board_length(&board_ref);
            let scorecard = get_scorecard_from_board(&board_ref);
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
        _ => {
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
    };

    Ok(response)
}

// Handle client registration
async fn handle_register(
    req: Request<hyper::body::Incoming>,
    client_registry: ClientRegistry,
    card_assignments: CardAssignments,
    client_cards: ClientCards,
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

    // Generate client ID
    let client_id = generate_client_id(&register_request.name, &register_request.client_type);

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
        let client_info = ClientInfo {
            id: client_id.clone(),
            name: register_request.name.clone(),
            client_type: register_request.client_type.clone(),
            registered_at: std::time::SystemTime::now(),
        };

        registry.insert(register_request.name.clone(), client_info);
        println!("✅ Client registered: {} (ID: {})", register_request.name, client_id);
    }

    // Check if client requested cards during registration
    if let Some(card_count) = register_request.nocard {
        println!("🎴 Generating {} cards for client '{}' during registration", card_count, register_request.name);
        
        // Generate the requested number of cards using the card generator
        let generator = TombolaGenerator::new();
        let cards = generator.generate_cards(card_count as usize);
        
        // Store the cards in the registries
        if let (Ok(mut assignments), Ok(mut client_cards_map)) = (card_assignments.lock(), client_cards.lock()) {
            let mut card_ids_for_client = Vec::new();
            
            for card_with_id in cards {
                let card_id_hex = format!("{:016X}", card_with_id.id);
                
                // Store card assignment
                assignments.insert(card_id_hex.clone(), CardAssignment {
                    card_id: card_id_hex.clone(),
                    client_id: client_id.clone(),
                    card_data: card_with_id.card,
                });
                
                card_ids_for_client.push(card_id_hex);
            }
            
            // Store card IDs for this client
            client_cards_map.insert(client_id.clone(), card_ids_for_client);
            
            println!("✅ Generated and assigned {} cards to client '{}'", card_count, register_request.name);
        } else {
            println!("⚠️  Failed to acquire card registry locks for client '{}'", register_request.name);
        }
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

// Function to get numbers from the board reference
fn get_numbers_from_board(board_ref: &Arc<Mutex<Board>>) -> Vec<Number> {
    if let Ok(board) = board_ref.lock() {
        board.get_numbers()
    } else {
        Vec::new()
    }
}

// Function to get the board length
fn get_board_length(board_ref: &Arc<Mutex<Board>>) -> usize {
    if let Ok(board) = board_ref.lock() {
        board.len()
    } else {
        0
    }
}

// Function to get the scorecard value from the board
fn get_scorecard_from_board(board_ref: &Arc<Mutex<Board>>) -> Number {
    if let Ok(board) = board_ref.lock() {
        board.get_scorecard()
    } else {
        0
    }
}

// Function to get pouch information
fn get_pouch_info(pouch_ref: &Arc<Mutex<Pouch>>) -> (Vec<Number>, usize) {
    if let Ok(pouch) = pouch_ref.lock() {
        (pouch.numbers.clone(), pouch.len())
    } else {
        (Vec::new(), 0)
    }
}

// Client ID generation function
fn generate_client_id(name: &str, client_type: &str) -> String {
    let mut hasher = DefaultHasher::new();
    
    // Hash the client info with current timestamp for uniqueness
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    
    hasher.write(name.as_bytes());
    hasher.write(client_type.as_bytes());
    hasher.write(&timestamp.to_be_bytes());
    
    format!("{:016X}", hasher.finish())
}

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
        error: format!("Client '{}' not found", client_name),
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
    card_assignments: CardAssignments,
    client_cards: ClientCards,
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
    if let Ok(client_cards_map) = client_cards.lock() {
        if let Some(existing_cards) = client_cards_map.get(&client_id) {
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

    // Generate cards using the TombolaGenerator
    let generator = TombolaGenerator::new();
    let cards_with_ids = generator.generate_cards(generate_request.count as usize);

    // Store card assignments
    let mut card_infos = Vec::new();
    if let (Ok(mut assignments), Ok(mut client_cards_map)) = (card_assignments.lock(), client_cards.lock()) {
        let client_card_ids = client_cards_map.entry(client_id.clone()).or_insert_with(Vec::new);
        
        for card_with_id in cards_with_ids {
            let card_id_str = format!("{:016X}", card_with_id.id);
            
            // Store the assignment
            assignments.insert(card_id_str.clone(), CardAssignment {
                card_id: card_id_str.clone(),
                client_id: client_id.clone(),
                card_data: card_with_id.card.clone(),
            });
            
            // Add to client's card list
            client_card_ids.push(card_id_str.clone());
            
            // Convert Card to CardInfo for response
            let card_info = CardInfo {
                card_id: card_id_str,
                card_data: card_with_id.card.iter().map(|row| {
                    row.iter().map(|cell| *cell).collect()
                }).collect(),
            };
            card_infos.push(card_info);
        }
    }

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
    client_cards: ClientCards,
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
    let assigned_cards = if let Ok(client_cards_map) = client_cards.lock() {
        client_cards_map.get(&client_id).cloned().unwrap_or_default()
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
    card_assignments: CardAssignments,
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
    let card_assignment = if let Ok(assignments) = card_assignments.lock() {
        assignments.get(&card_id).cloned()
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
            row.iter().map(|cell| *cell).collect()
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

// Generate client ID based on name and type
