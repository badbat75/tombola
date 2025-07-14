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

// Client information storage
#[derive(Debug, Clone)]
struct ClientInfo {
    id: String,
    name: String,
    client_type: String,
    registered_at: std::time::SystemTime,
}

// Global client registry (keyed by client name)
type ClientRegistry = Arc<Mutex<HashMap<String, ClientInfo>>>;

// Start the HTTP server with Tokio
pub fn start_server(board_ref: Arc<Mutex<Board>>, pouch_ref: Arc<Mutex<Pouch>>) -> (tokio::task::JoinHandle<()>, Arc<AtomicBool>) {
    let shutdown_signal = Arc::new(AtomicBool::new(false));
    let shutdown_clone = Arc::clone(&shutdown_signal);
    let client_registry: ClientRegistry = Arc::new(Mutex::new(HashMap::new()));
    
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
                    let io = TokioIo::new(stream);
                    
                    // Spawn a task to handle the connection
                    tokio::spawn(async move {
                        let service = service_fn(move |req| {
                            handle_request(req, Arc::clone(&board_clone), Arc::clone(&pouch_clone), Arc::clone(&registry_clone))
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
) -> Result<Response<Full<Bytes>>, Infallible> {
    let response = match (req.method(), req.uri().path()) {
        (&Method::POST, "/register") => {
            handle_register(req, client_registry).await
        }
        (&Method::GET, path) if path.starts_with("/client/") => {
            let client_name = &path[8..]; // Remove "/client/" prefix
            handle_client_info(client_name, client_registry).await
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
            println!("🔄 Client already registered: {} (ID: {})", register_request.name, existing_client.id);
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
