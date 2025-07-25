use std::sync::Arc;
use std::net::SocketAddr;

use axum::{
    routing::{get, post},
    Router,
};
use tower_http::cors::CorsLayer;

use crate::config::ServerConfig;
use crate::logging::{log, LogLevel};
use crate::game::GameRegistry;
use crate::client::ClientRegistry;
use crate::api_handlers::{handle_global_clientinfo, handle_global_clientinfo_by_id, handle_global_register, handle_global_gameslist, handle_global_newgame, handle_join, handle_generatecards, handle_listassignedcards, handle_getassignedcard, handle_board, handle_pouch, handle_scoremap, handle_status, handle_extract, handle_dumpgame};

const MODULE_NAME: &str = "server";

pub struct AppState {
    pub game_registry: GameRegistry,
    pub global_client_registry: ClientRegistry,
    pub config: ServerConfig,
}

#[must_use] pub fn start_server(config: ServerConfig) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        // Create the GameRegistry (no initial game)
        let game_registry = GameRegistry::new();

        let app_state = Arc::new(AppState {
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
                log(LogLevel::Error, MODULE_NAME, &format!("Failed to start API server: {e}"));
                return;
            }
        };

        // Use axum::serve to handle the server (without internal graceful shutdown)
        if let Err(err) = axum::serve(listener, app).await {
            log(LogLevel::Error, MODULE_NAME, &format!("Server error: {err:?}"));
        }

        log(LogLevel::Info, MODULE_NAME, "Server shutdown complete");
    })
}
