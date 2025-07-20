use std::sync::Arc;
use std::net::SocketAddr;
use std::sync::atomic::AtomicBool;

use axum::{
    routing::{get, post},
    Router,
};
use tower_http::cors::CorsLayer;

use crate::config::ServerConfig;
use crate::logging::{log_info, log_error, log_error_stderr};
use crate::game::Game;
use crate::api_handlers::*;

// Response structures for JSON serialization
#[derive(serde::Serialize)]
#[allow(dead_code)]
struct ErrorResponse {
    error: String,
}

pub struct AppState {
    pub game: Game,
    pub config: ServerConfig,
}

pub fn start_server(config: ServerConfig) -> (tokio::task::JoinHandle<()>, Arc<AtomicBool>) {
    let shutdown_signal = Arc::new(AtomicBool::new(false));
    let _shutdown_clone = Arc::clone(&shutdown_signal);
    
    // Create the unified Game state container
    let game = Game::new();
    log_info(&format!("Created new game instance: {}", game.game_info()));
    
    let handle = tokio::spawn(async move {
        let app_state = Arc::new(AppState {
            game,
            config: config.clone(),
        });

        let app = Router::new()
            .route("/register", post(handle_register))
            .route("/clientinfo", get(handle_client_info))
            .route("/clientinfo/{client_id}", get(handle_client_info_by_id))
            .route("/generatecards", post(handle_generate_cards))
            .route("/listassignedcards", get(handle_list_assigned_cards))
            .route("/getassignedcard/{card_id}", get(handle_get_assigned_card))
            .route("/board", get(handle_board))
            .route("/pouch", get(handle_pouch))
            .route("/scoremap", get(handle_scoremap))
            .route("/status", get(handle_status))
            .route("/runninggameid", get(handle_running_game_id))
            .route("/extract", post(handle_extract))
            .route("/newgame", post(handle_newgame))
            .route("/dumpgame", post(handle_dumpgame))
            .layer(CorsLayer::permissive())
            .with_state(app_state);

        let addr = SocketAddr::from((config.host.parse::<std::net::IpAddr>().unwrap_or([127, 0, 0, 1].into()), config.port));
        let listener = match tokio::net::TcpListener::bind(&addr).await {
            Ok(listener) => listener,
            Err(e) => {
                log_error_stderr(&format!("Failed to start API server: {e}"));
                return;
            }
        };

        log_info(&format!("Server starting on {addr}"));
        
        // Use axum::serve to handle the server
        if let Err(err) = axum::serve(listener, app).await {
            log_error(&format!("Server error: {err:?}"));
        }
        
        log_info("Server shutdown complete");
    });

    (handle, shutdown_signal)
}
