use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::convert::Infallible;
use std::net::SocketAddr;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{body::Bytes, Request, Response, StatusCode, Method};
use hyper_util::rt::TokioIo;
use tokio::net::TcpListener;
use http_body_util::Full;
use serde_json::json;
use crate::defs::NumberEntry;

// Response structures for JSON serialization
#[derive(serde::Serialize)]
struct LastNumberResponse {
    last_number: u8,
}

#[derive(serde::Serialize)]
struct BoardResponse {
    board: Vec<u8>,
}

#[derive(serde::Serialize)]
struct ErrorResponse {
    error: String,
}

// Start the HTTP server with Tokio
pub fn start_server(board_ref: Arc<Mutex<Vec<NumberEntry>>>) -> (tokio::task::JoinHandle<()>, Arc<AtomicBool>) {
    let shutdown_signal = Arc::new(AtomicBool::new(false));
    let shutdown_clone = Arc::clone(&shutdown_signal);
    
    let handle = tokio::spawn(async move {
        let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
        let listener = match TcpListener::bind(&addr).await {
            Ok(listener) => {
                println!("API Server started on http://127.0.0.1:3000");
                listener
            }
            Err(e) => {
                eprintln!("Failed to start API server: {}", e);
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
                    let io = TokioIo::new(stream);
                    
                    // Spawn a task to handle the connection
                    tokio::spawn(async move {
                        let service = service_fn(move |req| {
                            handle_request(req, Arc::clone(&board_clone))
                        });
                        
                        if let Err(err) = http1::Builder::new()
                            .serve_connection(io, service)
                            .await
                        {
                            eprintln!("Error serving connection: {:?}", err);
                        }
                    });
                }
                Ok(Err(e)) => {
                    eprintln!("Error accepting connection: {}", e);
                    break;
                }
                Err(_) => {
                    // Timeout occurred, continue to check shutdown signal
                    continue;
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
    board_ref: Arc<Mutex<Vec<NumberEntry>>>,
) -> Result<Response<Full<Bytes>>, Infallible> {
    let response = match (req.method(), req.uri().path()) {
        (&Method::GET, "/last_number") => {
            match get_last_number(&board_ref) {
                Some(number) => {
                    let response = LastNumberResponse { last_number: number };
                    let body = serde_json::to_string(&response).unwrap_or_else(|_| "{}".to_string());
                    Response::builder()
                        .status(StatusCode::OK)
                        .header("Content-Type", "application/json")
                        .header("Access-Control-Allow-Origin", "*")
                        .body(Full::new(Bytes::from(body)))
                        .unwrap()
                }
                None => {
                    let response = ErrorResponse {
                        error: "No number extracted yet".to_string(),
                    };
                    let body = serde_json::to_string(&response).unwrap_or_else(|_| "{}".to_string());
                    Response::builder()
                        .status(StatusCode::NOT_FOUND)
                        .header("Content-Type", "application/json")
                        .header("Access-Control-Allow-Origin", "*")
                        .body(Full::new(Bytes::from(body)))
                        .unwrap()
                }
            }
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
        (&Method::GET, "/status") => {
            let board_len = get_board_length(&board_ref);
            let response = json!({
                "status": "running",
                "numbers_extracted": board_len,
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

// Function to get the last extracted number from the board reference
fn get_last_number(board_ref: &Arc<Mutex<Vec<NumberEntry>>>) -> Option<u8> {
    if let Ok(board) = board_ref.lock() {
        board.last().map(|entry| entry.number)
    } else {
        None
    }
}

// Function to get numbers from the board reference
fn get_numbers_from_board(board_ref: &Arc<Mutex<Vec<NumberEntry>>>) -> Vec<u8> {
    if let Ok(board) = board_ref.lock() {
        board.iter().map(|entry| entry.number).collect()
    } else {
        Vec::new()
    }
}

// Function to get the board length
fn get_board_length(board_ref: &Arc<Mutex<Vec<NumberEntry>>>) -> usize {
    if let Ok(board) = board_ref.lock() {
        board.len()
    } else {
        0
    }
}
