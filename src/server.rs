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
use crate::game::{Game, GameRegistry};
use crate::client::ClientRegistry;
use crate::api_handlers::*;

// Response structures for JSON serialization
#[derive(serde::Serialize)]
#[allow(dead_code)]
struct ErrorResponse {
    error: String,
}

pub struct AppState {
    pub game: Game,
    pub game_registry: GameRegistry,
    pub global_client_registry: ClientRegistry,
    pub config: ServerConfig,
}

pub fn start_server(config: ServerConfig) -> (tokio::task::JoinHandle<()>, Arc<AtomicBool>) {
    let shutdown_signal = Arc::new(AtomicBool::new(false));
    let _shutdown_clone = Arc::clone(&shutdown_signal);

    // Create the unified Game state container
    let game = Game::new();
    log_info(&format!("Created new game instance: {}", game.game_info()));

    // Create the GameRegistry and register the initial game
    let game_registry = GameRegistry::new();
    let game_arc = Arc::new(game.clone());
    if let Err(e) = game_registry.add_game(game_arc) {
        log_error(&format!("Failed to register initial game: {e}"));
    } else {
        log_info(&format!("Registered initial game in registry: {}", game.id()));
    }

    let handle = tokio::spawn(async move {
        let app_state = Arc::new(AppState {
            game,
            game_registry,
            global_client_registry: ClientRegistry::new(),
            config: config.clone(),
        });

        let app = Router::new()
            // Client & Game Management routes
            .route("/clientinfo", get(handle_global_clientinfo))
            .route("/clientinfo/{client_id}", get(handle_global_clientinfo_by_id))
            .route("/register", post(handle_global_register))
            .route("/gameslist", get(handle_global_gameslist))
            .route("/newgame", post(handle_global_newgame))
            // Game Functions routes
            .route("/{game_id}/join", post(handle_join))
            .route("/{game_id}/generatecards", post(handle_generatecards))
            .route("/{game_id}/listassignedcards", get(handle_listassignedcards))
            .route("/{game_id}/getassignedcard/{card_id}", get(handle_getassignedcard))
            .route("/{game_id}/board", get(handle_board))
            .route("/{game_id}/pouch", get(handle_pouch))
            .route("/{game_id}/scoremap", get(handle_scoremap))
            .route("/{game_id}/status", get(handle_status))
            .route("/{game_id}/extract", post(handle_extract))
            .route("/{game_id}/dumpgame", post(handle_dumpgame))
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
