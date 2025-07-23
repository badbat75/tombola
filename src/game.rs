// src/game.rs
// This module provides a unified Game struct that encapsulates all game state components
// and provides coordinated access to prevent deadlocks and simplify state management.
//
// The Game struct supports complete state destruction via reset_game(), which destroys
// all persistent data including client sessions and card assignments, forcing complete
// re-registration for a truly fresh game experience.

use std::sync::{Arc, Mutex};
use std::time::SystemTime;
use std::collections::HashMap;
use rand::Rng;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use crate::board::Board;
use crate::pouch::Pouch;
use crate::score::ScoreCard;
use crate::logging::log_warning;
use std::collections::HashSet;
use crate::card::CardAssignmentManager;
use crate::defs::Number;
use crate::extraction::perform_extraction;

/// Game-specific client type association
/// This allows clients to have different types in different games
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameClientType {
    pub client_id: String,
    pub client_type: String, // "board", "player", etc.
}

/// Game-specific client type registry
/// Manages client types within a specific game context
#[derive(Debug, Clone)]
pub struct GameClientTypeRegistry {
    /// HashMap mapping client_id -> client_type for this specific game
    client_types: Arc<Mutex<HashMap<String, String>>>,
}

impl GameClientTypeRegistry {
    /// Create a new empty client type registry
    pub fn new() -> Self {
        Self {
            client_types: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Set the client type for a client in this game
    pub fn set_client_type(&self, client_id: &str, client_type: &str) -> Result<(), String> {
        let mut types_lock = self.client_types.lock()
            .map_err(|_| "Failed to lock client types registry")?;
        
        types_lock.insert(client_id.to_string(), client_type.to_string());
        Ok(())
    }

    /// Get the client type for a client in this game
    pub fn get_client_type(&self, client_id: &str) -> Result<Option<String>, String> {
        let types_lock = self.client_types.lock()
            .map_err(|_| "Failed to lock client types registry")?;
        
        Ok(types_lock.get(client_id).cloned())
    }

    /// Remove a client's type association from this game
    pub fn remove_client_type(&self, client_id: &str) -> Result<Option<String>, String> {
        let mut types_lock = self.client_types.lock()
            .map_err(|_| "Failed to lock client types registry")?;
        
        Ok(types_lock.remove(client_id))
    }

    /// Get all clients of a specific type in this game
    pub fn get_clients_by_type(&self, client_type: &str) -> Result<Vec<String>, String> {
        let types_lock = self.client_types.lock()
            .map_err(|_| "Failed to lock client types registry")?;
        
        let clients: Vec<String> = types_lock
            .iter()
            .filter(|(_, ctype)| *ctype == client_type)
            .map(|(client_id, _)| client_id.clone())
            .collect();
        
        Ok(clients)
    }

    /// Check if a client has a specific type in this game
    pub fn is_client_type(&self, client_id: &str, client_type: &str) -> Result<bool, String> {
        let types_lock = self.client_types.lock()
            .map_err(|_| "Failed to lock client types registry")?;
        
        Ok(types_lock.get(client_id).map_or(false, |ctype| ctype == client_type))
    }

    /// Get all client type associations in this game
    pub fn get_all_client_types(&self) -> Result<Vec<GameClientType>, String> {
        let types_lock = self.client_types.lock()
            .map_err(|_| "Failed to lock client types registry")?;
        
        let client_types: Vec<GameClientType> = types_lock
            .iter()
            .map(|(client_id, client_type)| GameClientType {
                client_id: client_id.clone(),
                client_type: client_type.clone(),
            })
            .collect();
        
        Ok(client_types)
    }

    /// Clear all client type associations for this game
    pub fn clear(&self) -> Result<usize, String> {
        let mut types_lock = self.client_types.lock()
            .map_err(|_| "Failed to lock client types registry")?;
        
        let count = types_lock.len();
        types_lock.clear();
        Ok(count)
    }
}

/// Represents the current status of a game
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum GameStatus {
    /// New game with no numbers extracted yet
    New,
    /// Active game with at least one number extracted
    Active,
    /// Closed game where BINGO has been reached
    Closed,
}

impl GameStatus {
    /// Convert GameStatus to a string representation
    pub fn as_str(&self) -> &'static str {
        match self {
            GameStatus::New => "New",
            GameStatus::Active => "Active",
            GameStatus::Closed => "Closed",
        }
    }
}

/// Represents a game entry in the registry
#[derive(Debug, Clone)]
pub struct GameEntry {
    /// The game ID
    pub game_id: String,
    /// Reference to the actual game instance
    pub game: Arc<Game>,
    /// When this game was registered
    pub registered_at: SystemTime,
    /// When this game was closed (if applicable)
    pub closed_at: Option<SystemTime>,
}

impl GameEntry {
    /// Create a new game entry
    pub fn new(game_id: String, game: Arc<Game>) -> Self {
        Self {
            game_id,
            game,
            registered_at: SystemTime::now(),
            closed_at: None,
        }
    }

    /// Get the current status of this game
    pub fn status(&self) -> GameStatus {
        if self.game.is_bingo_reached() {
            GameStatus::Closed
        } else if self.game.has_game_started() {
            GameStatus::Active
        } else {
            GameStatus::New
        }
    }

    /// Update the closed_at timestamp if the game is closed
    /// This should be called when checking status to ensure closed_at is properly set
    pub fn update_closed_at(&mut self) {
        if self.game.is_bingo_reached() && self.closed_at.is_none() {
            self.closed_at = Some(SystemTime::now());
        }
    }

    /// Get the status and update closed_at if necessary
    /// This is a convenience method that combines status checking with closed_at updating
    pub fn status_with_update(&mut self) -> GameStatus {
        let status = self.status();
        if status == GameStatus::Closed && self.closed_at.is_none() {
            self.closed_at = Some(SystemTime::now());
        }
        status
    }

    /// Check if the game is closed
    pub fn is_closed(&self) -> bool {
        self.game.is_bingo_reached()
    }

    /// Get the closed_at time as a human-readable string
    pub fn closed_at_string(&self) -> Option<String> {
        self.closed_at.map(|closed_at| {
            match closed_at.duration_since(std::time::UNIX_EPOCH) {
                Ok(duration) => {
                    let datetime: DateTime<Utc> = DateTime::from_timestamp(duration.as_secs() as i64, 0)
                        .unwrap_or_else(Utc::now);
                    datetime.format("%Y-%m-%d %H:%M:%S UTC").to_string()
                }
                Err(_) => "Unknown time".to_string(),
            }
        })
    }

    /// Get game info as a formatted string
    pub fn info(&self) -> String {
        let closed_info = match self.closed_at_string() {
            Some(closed_time) => format!(", closed_at={closed_time}"),
            None => String::new(),
        };

        format!(
            "GameEntry[id={}, status={}, board_len={}, score={}, registered_at={}{}]",
            self.game_id,
            self.status().as_str(),
            self.game.board_length(),
            self.game.published_score(),
            self.registered_at_string(),
            closed_info
        )
    }

    /// Get a human-readable registration time string
    pub fn registered_at_string(&self) -> String {
        match self.registered_at.duration_since(std::time::UNIX_EPOCH) {
            Ok(duration) => {
                let datetime: DateTime<Utc> = DateTime::from_timestamp(duration.as_secs() as i64, 0)
                    .unwrap_or_else(Utc::now);
                datetime.format("%Y-%m-%d %H:%M:%S UTC").to_string()
            }
            Err(_) => "Unknown time".to_string(),
        }
    }
}

/// Registry for managing multiple games
/// This allows the server to track multiple concurrent or historical games
///
/// # Example
/// ```
/// use std::sync::Arc;
/// use tombola::game::{Game, GameRegistry, GameStatus};
///
/// // Create a new registry
/// let registry = GameRegistry::new();
///
/// // Create and add games
/// let game1 = Arc::new(Game::new());
/// let game2 = Arc::new(Game::new());
///
/// let game1_id = registry.add_game(game1.clone()).unwrap();
/// let game2_id = registry.add_game(game2.clone()).unwrap();
///
/// // List all games
/// let games = registry.games_list().unwrap();
/// for (id, status, info) in games {
///     println!("Game {}: {} - {}", id, status.as_str(), info);
/// }
///
/// // Get games by status
/// let new_games = registry.games_by_status(GameStatus::New).unwrap();
/// println!("New games: {:?}", new_games);
///
/// // Get status summary
/// let (new_count, active_count, closed_count) = registry.status_summary().unwrap();
/// println!("Status: {} new, {} active, {} closed", new_count, active_count, closed_count);
/// ```
#[derive(Debug)]
pub struct GameRegistry {
    /// HashMap storing game entries by game ID
    games: Arc<Mutex<HashMap<String, GameEntry>>>,
}

impl GameRegistry {
    /// Create a new empty game registry
    pub fn new() -> Self {
        Self {
            games: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Add a new game to the registry
    /// Returns the game ID if successful, or an error message
    pub fn add_game(&self, game: Arc<Game>) -> Result<String, String> {
        let game_id = game.id();

        let mut games_lock = self.games.lock()
            .map_err(|_| "Failed to lock games registry")?;

        // Check if game ID already exists
        if games_lock.contains_key(&game_id) {
            return Err(format!("Game with ID '{game_id}' already exists in registry"));
        }

        let entry = GameEntry::new(game_id.clone(), game);
        games_lock.insert(game_id.clone(), entry);

        Ok(game_id)
    }

    /// Get a list of all registered games with their status
    /// Returns a vector of tuples: (game_id, status, game_info)
    pub fn games_list(&self) -> Result<Vec<(String, GameStatus, String)>, String> {
        let mut games_lock = self.games.lock()
            .map_err(|_| "Failed to lock games registry")?;

        let mut games_info = Vec::new();

        for (game_id, entry) in games_lock.iter_mut() {
            let status = entry.status_with_update(); // This will update closed_at if necessary
            let info = entry.info();
            games_info.push((game_id.clone(), status, info));
        }

        // Sort by game ID for consistent ordering
        games_info.sort_by(|a, b| a.0.cmp(&b.0));

        Ok(games_info)
    }

    /// Get a specific game by ID
    pub fn get_game(&self, game_id: &str) -> Result<Option<Arc<Game>>, String> {
        let games_lock = self.games.lock()
            .map_err(|_| "Failed to lock games registry")?;

        Ok(games_lock.get(game_id).map(|entry| entry.game.clone()))
    }

    /// Remove a game from the registry
    /// Returns true if the game was removed, false if it didn't exist
    pub fn remove_game(&self, game_id: &str) -> Result<bool, String> {
        let mut games_lock = self.games.lock()
            .map_err(|_| "Failed to lock games registry")?;

        Ok(games_lock.remove(game_id).is_some())
    }

    /// Get the total number of registered games
    pub fn total_games(&self) -> Result<usize, String> {
        let games_lock = self.games.lock()
            .map_err(|_| "Failed to lock games registry")?;

        Ok(games_lock.len())
    }

    /// Get games by status
    pub fn games_by_status(&self, status: GameStatus) -> Result<Vec<String>, String> {
        let mut games_lock = self.games.lock()
            .map_err(|_| "Failed to lock games registry")?;

        let mut matching_games = Vec::new();

        for (game_id, entry) in games_lock.iter_mut() {
            if entry.status_with_update() == status { // This will update closed_at if necessary
                matching_games.push(game_id.clone());
            }
        }

        matching_games.sort();
        Ok(matching_games)
    }

    /// Get a summary of games by status
    pub fn status_summary(&self) -> Result<(usize, usize, usize), String> {
        let mut games_lock = self.games.lock()
            .map_err(|_| "Failed to lock games registry")?;

        let mut new_count = 0;
        let mut active_count = 0;
        let mut closed_count = 0;

        for entry in games_lock.values_mut() {
            match entry.status_with_update() { // This will update closed_at if necessary
                GameStatus::New => new_count += 1,
                GameStatus::Active => active_count += 1,
                GameStatus::Closed => closed_count += 1,
            }
        }

        Ok((new_count, active_count, closed_count))
    }

    /// Clear all games from the registry
    pub fn clear(&self) -> Result<usize, String> {
        let mut games_lock = self.games.lock()
            .map_err(|_| "Failed to lock games registry")?;

        let count = games_lock.len();
        games_lock.clear();
        Ok(count)
    }
}

impl Default for GameRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for GameRegistry {
    fn clone(&self) -> Self {
        Self {
            games: self.games.clone(),
        }
    }
}

/// Game struct that holds all shared game state components
/// This provides a single point of access for all game operations
/// and ensures proper mutex coordination to prevent deadlocks.
#[derive(Clone, Debug)]
pub struct Game {
    id: Arc<Mutex<String>>,
    created_at: Arc<Mutex<SystemTime>>,
    board: Arc<Mutex<Board>>,
    pouch: Arc<Mutex<Pouch>>,
    scorecard: Arc<Mutex<ScoreCard>>,
    registered_clients: Arc<Mutex<HashSet<String>>>,  // Just store client IDs
    card_manager: Arc<Mutex<CardAssignmentManager>>,
    client_type_registry: GameClientTypeRegistry,  // Game-specific client types
}

impl Game {
    /// Create a new Game instance with all components initialized
    pub fn new() -> Self {
        // Generate a random game ID
        let mut rng = rand::rng();
        let game_id = format!("game_{:08x}", rng.random::<u32>());

        Self {
            id: Arc::new(Mutex::new(game_id)),
            created_at: Arc::new(Mutex::new(SystemTime::now())),
            board: Arc::new(Mutex::new(Board::new())),
            pouch: Arc::new(Mutex::new(Pouch::new())),
            scorecard: Arc::new(Mutex::new(ScoreCard::new())),
            registered_clients: Arc::new(Mutex::new(HashSet::new())),
            card_manager: Arc::new(Mutex::new(CardAssignmentManager::new())),
            client_type_registry: GameClientTypeRegistry::new(),
        }
    }

    /// Get the game ID
    pub fn id(&self) -> String {
        self.id.lock().unwrap().clone()
    }

    /// Get the game creation time
    pub fn created_at(&self) -> SystemTime {
        *self.created_at.lock().unwrap()
    }

    /// Get a human-readable creation time string
    pub fn created_at_string(&self) -> String {
        let created_at = *self.created_at.lock().unwrap();
        match created_at.duration_since(std::time::UNIX_EPOCH) {
            Ok(duration) => {
                let datetime: DateTime<Utc> = DateTime::from_timestamp(duration.as_secs() as i64, 0)
                    .unwrap_or_else(Utc::now);
                datetime.format("%Y-%m-%d %H:%M:%S UTC").to_string()
            }
            Err(_) => "Unknown time".to_string(),
        }
    }

    /// Get a reference to the board Arc<Mutex<Board>>
    pub fn board(&self) -> &Arc<Mutex<Board>> {
        &self.board
    }

    /// Get a reference to the pouch Arc<Mutex<Pouch>>
    pub fn pouch(&self) -> &Arc<Mutex<Pouch>> {
        &self.pouch
    }

    /// Get a reference to the scorecard Arc<Mutex<ScoreCard>>
    pub fn scorecard(&self) -> &Arc<Mutex<ScoreCard>> {
        &self.scorecard
    }

    /// Get a reference to the registered clients Arc<Mutex<HashSet<String>>>
    pub fn registered_clients(&self) -> &Arc<Mutex<HashSet<String>>> {
        &self.registered_clients
    }

    /// Get all registered client information from the global registry
    pub fn get_registered_client_infos(&self, client_registry: &crate::client::ClientRegistry) -> Result<Vec<crate::client::ClientInfo>, String> {
        let client_ids = if let Ok(clients) = self.registered_clients.lock() {
            clients.iter().cloned().collect::<Vec<String>>()
        } else {
            return Err("Failed to lock registered clients".to_string());
        };

        let mut client_infos = Vec::new();
        for client_id in client_ids {
            match client_registry.get(&client_id) {
                Ok(Some(client_info)) => client_infos.push(client_info),
                Ok(None) => {
                    // Client ID exists in game but not in global registry - this shouldn't happen
                    log_warning(&format!("Client ID {} registered in game but not found in global registry", client_id));
                }
                Err(e) => {
                    return Err(format!("Failed to get client info for {}: {}", client_id, e));
                }
            }
        }
        
        Ok(client_infos)
    }

    /// Check if a specific client is registered and get their info
    pub fn get_client_info(&self, client_id: &str, client_registry: &crate::client::ClientRegistry) -> Result<Option<crate::client::ClientInfo>, String> {
        // First check if client is registered in this game
        let is_registered = if let Ok(clients) = self.registered_clients.lock() {
            clients.contains(client_id)
        } else {
            return Err("Failed to lock registered clients".to_string());
        };

        if !is_registered {
            return Ok(None);
        }

        // Get client info from global registry
        client_registry.get(client_id)
    }

    /// Get the count of registered clients
    pub fn registered_client_count(&self) -> Result<usize, String> {
        if let Ok(clients) = self.registered_clients.lock() {
            Ok(clients.len())
        } else {
            Err("Failed to lock registered clients".to_string())
        }
    }

    /// Get list of registered client IDs
    pub fn get_registered_client_ids(&self) -> Result<Vec<String>, String> {
        if let Ok(clients) = self.registered_clients.lock() {
            Ok(clients.iter().cloned().collect())
        } else {
            Err("Failed to lock registered clients".to_string())
        }
    }

    /// Add a client to this game (only if no numbers have been extracted)
    pub fn add_client(&self, client_id: String) -> Result<bool, String> {
        let numbers_extracted = self.has_game_started();
        if numbers_extracted {
            return Err("Cannot register new clients after numbers have been extracted".to_string());
        }

        if let Ok(mut clients) = self.registered_clients.lock() {
            Ok(clients.insert(client_id))
        } else {
            Err("Failed to lock registered clients".to_string())
        }
    }

    /// Check if a client is registered to this game
    pub fn contains_client(&self, client_id: &str) -> bool {
        if let Ok(clients) = self.registered_clients.lock() {
            clients.contains(client_id)
        } else {
            false
        }
    }

    // Game-specific client type management methods

    /// Set the client type for a client in this specific game
    pub fn set_client_type(&self, client_id: &str, client_type: &str) -> Result<(), String> {
        self.client_type_registry.set_client_type(client_id, client_type)
    }

    /// Get the client type for a client in this specific game
    pub fn get_client_type(&self, client_id: &str) -> Result<Option<String>, String> {
        self.client_type_registry.get_client_type(client_id)
    }

    /// Check if a client has a specific type in this game
    pub fn is_client_type(&self, client_id: &str, client_type: &str) -> Result<bool, String> {
        self.client_type_registry.is_client_type(client_id, client_type)
    }

    /// Get all clients of a specific type in this game
    pub fn get_clients_by_type(&self, client_type: &str) -> Result<Vec<String>, String> {
        self.client_type_registry.get_clients_by_type(client_type)
    }

    /// Get all client type associations in this game
    pub fn get_all_client_types(&self) -> Result<Vec<GameClientType>, String> {
        self.client_type_registry.get_all_client_types()
    }

    /// Remove a client's type association from this game
    pub fn remove_client_type(&self, client_id: &str) -> Result<Option<String>, String> {
        self.client_type_registry.remove_client_type(client_id)
    }

    /// Get a reference to the card manager Arc<Mutex<CardAssignmentManager>>
    pub fn card_manager(&self) -> &Arc<Mutex<CardAssignmentManager>> {
        &self.card_manager
    }

    /// Perform a number extraction using the coordinated extraction logic
    /// This encapsulates the complex mutex coordination required for extraction
    pub fn extract_number(&self, current_working_score: Number, board_client_id: Option<&str>) -> Result<(Number, Number), String> {
        perform_extraction(
            &self.pouch,
            &self.board,
            &self.scorecard,
            &self.card_manager,
            current_working_score,
            board_client_id,
        )
    }

    /// Check if the game has started (any numbers extracted)
    pub fn has_game_started(&self) -> bool {
        if let Ok(board) = self.board.lock() {
            !board.is_empty()
        } else {
            // If we can't access the board, assume game has started for safety
            true
        }
    }

    /// Get the current board length (number of extracted numbers)
    pub fn board_length(&self) -> usize {
        if let Ok(board) = self.board.lock() {
            board.len()
        } else {
            0
        }
    }

    /// Get the current published score from the scorecard
    pub fn published_score(&self) -> Number {
        if let Ok(scorecard) = self.scorecard.lock() {
            scorecard.published_score
        } else {
            0
        }
    }

    /// Check if BINGO has been reached (game over condition)
    pub fn is_bingo_reached(&self) -> bool {
        self.published_score() >= 15
    }

    /// Get the number of remaining numbers in the pouch
    pub fn pouch_length(&self) -> usize {
        if let Ok(pouch) = self.pouch.lock() {
            pouch.len()
        } else {
            0
        }
    }

    /// Check if the pouch is empty
    pub fn is_pouch_empty(&self) -> bool {
        self.pouch_length() == 0
    }

    /// Check if the game has ended (either BINGO reached or pouch empty)
    pub fn is_game_ended(&self) -> bool {
        self.is_bingo_reached() || self.is_pouch_empty()
    }

    /// Get the number of registered players
    pub fn player_count(&self) -> usize {
        if let Ok(clients) = self.registered_clients.lock() {
            clients.len()
        } else {
            0
        }
    }

    /// Get the total number of cards assigned in this game
    pub fn card_count(&self) -> usize {
        if let Ok(manager) = self.card_manager.lock() {
            manager.get_all_assignments().len()
        } else {
            0
        }
    }

    /// Get the current game status
    pub fn status(&self) -> GameStatus {
        if self.is_bingo_reached() {
            GameStatus::Closed
        } else if self.has_game_started() {
            GameStatus::Active
        } else {
            GameStatus::New
        }
    }

    /// Get running game ID and creation details
    pub fn get_running_game_info(&self) -> (String, String, SystemTime) {
        (
            self.id().to_string(),
            self.created_at_string(),
            self.created_at()
        )
    }

    /// Get game information as a formatted string for debugging/logging
    pub fn game_info(&self) -> String {
        format!(
            "Game[id={}, created={}, board_len={}, pouch_len={}, score={}, started={}]",
            self.id(),
            self.created_at_string(),
            self.board_length(),
            self.pouch_length(),
            self.published_score(),
            self.has_game_started()
        )
    }

    /// Dump the complete game state to a JSON file in data/games directory
    /// This function is called when the game ends (BINGO reached)
    pub fn dump_to_json(&self) -> Result<String, String> {
        use std::fs;
        use std::path::Path;

        // Create the serializable game state
        let game_state = match self.create_serializable_state() {
            Ok(state) => state,
            Err(e) => return Err(format!("Failed to create serializable state: {e}")),
        };

        // Create the filename with game ID
        let filename = format!("{}.json", self.id(),);
        let filepath = Path::new("data/games").join(&filename);

        // Ensure the directory exists
        if let Some(parent) = filepath.parent() {
            if let Err(e) = fs::create_dir_all(parent) {
                return Err(format!("Failed to create directory {parent:?}: {e}"));
            }
        }

        // Serialize the game state to JSON
        let json_content = match serde_json::to_string_pretty(&game_state) {
            Ok(json) => json,
            Err(e) => return Err(format!("Failed to serialize game state: {e}")),
        };

        // Write to file
        match fs::write(&filepath, json_content) {
            Ok(_) => Ok(format!("Game dumped to: {}", filepath.display())),
            Err(e) => Err(format!("Failed to write file {filepath:?}: {e}")),
        }
    }

    /// Dump the game state if the game has ended, otherwise return an error
    pub fn dump_if_ended(&self) -> Result<String, String> {
        if self.is_game_ended() {
            self.dump_to_json()
        } else {
            Err("Game has not ended yet (BINGO not reached and pouch not empty)".to_string())
        }
    }

    /// Create a serializable version of the game state
    fn create_serializable_state(&self) -> Result<SerializableGameState, String> {
        let board = {
            let guard = self.board.lock()
                .map_err(|_| "Failed to lock board")?;
            guard.clone()
        };

        let pouch = {
            let guard = self.pouch.lock()
                .map_err(|_| "Failed to lock pouch")?;
            guard.clone()
        };

        let scorecard = {
            let guard = self.scorecard.lock()
                .map_err(|_| "Failed to lock scorecard")?;
            guard.clone()
        };

        let registered_clients = {
            let guard = self.registered_clients.lock()
                .map_err(|_| "Failed to lock registered clients")?;
            guard.clone()
        };

        let card_manager = {
            let guard = self.card_manager.lock()
                .map_err(|_| "Failed to lock card manager")?;
            guard.clone()
        };

        Ok(SerializableGameState {
            id: self.id(),
            created_at: self.created_at(),
            board,
            pouch,
            scorecard,
            registered_clients,
            card_manager,
            game_ended_at: SystemTime::now(),
        })
    }
}

/// Serializable version of the Game struct for JSON dumping
#[derive(Serialize, Deserialize)]
pub struct SerializableGameState {
    pub id: String,
    pub created_at: SystemTime,
    pub board: Board,
    pub pouch: Pouch,
    pub scorecard: ScoreCard,
    pub registered_clients: HashSet<String>,
    pub card_manager: CardAssignmentManager,
    pub game_ended_at: SystemTime,
}

impl Default for Game {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_game_creation() {
        let game = Game::new();

        // Verify all components are properly initialized
        assert_eq!(game.board_length(), 0);
        assert_eq!(game.published_score(), 0);
        assert_eq!(game.pouch_length(), 90); // Should start with full pouch
        assert!(!game.has_game_started());
        assert!(!game.is_bingo_reached());
        assert!(!game.is_pouch_empty());

        // Verify new fields are set
        assert!(!game.id().is_empty());
        assert!(game.id().starts_with("game_"));
        assert_eq!(game.id().len(), 13); // "game_" + 8 hex chars

        // Verify creation time is recent (within last second)
        let now = SystemTime::now();
        let creation_time = game.created_at();
        let time_diff = now.duration_since(creation_time).unwrap_or_default();
        assert!(time_diff.as_secs() < 2); // Should be created within last 2 seconds

        // Verify the human-readable time string format
        let time_string = game.created_at_string();
        assert!(time_string.contains("UTC"));
        assert!(time_string.len() > 10); // Should be a reasonable length
    }

    #[test]
    fn test_game_state_queries() {
        let game = Game::new();

        // Test initial state
        assert_eq!(game.board_length(), 0);
        assert_eq!(game.published_score(), 0);
        assert!(!game.has_game_started());
        assert!(!game.is_bingo_reached());
    }

    #[test]
    fn test_unique_game_ids() {
        let game1 = Game::new();
        let game2 = Game::new();

        // Verify each game gets a unique ID
        assert_ne!(game1.id(), game2.id());

        // Verify both IDs follow the expected format
        assert!(game1.id().starts_with("game_"));
        assert!(game2.id().starts_with("game_"));
        assert_eq!(game1.id().len(), 13);
        assert_eq!(game2.id().len(), 13);
    }

    #[test]
    fn test_game_info() {
        let game = Game::new();
        let info = game.game_info();

        // Verify the info string contains expected components
        assert!(info.contains("Game[id="));
        assert!(info.contains("created="));
        assert!(info.contains("board_len=0"));
        assert!(info.contains("pouch_len=90"));
        assert!(info.contains("score=0"));
        assert!(info.contains("started=false"));
        assert!(info.contains(&game.id()));
    }

    #[test]
    fn test_game_state_serialization() {
        let game = Game::new();

        // Test creating serializable state
        let serializable_state = game.create_serializable_state();
        assert!(serializable_state.is_ok());

        let state = serializable_state.unwrap();
        assert_eq!(state.id, game.id());
        assert_eq!(state.board.len(), 0);
        assert_eq!(state.pouch.len(), 90);
    }

    #[test]
    fn test_game_ending_conditions() {
        let game = Game::new();

        // Initially, game hasn't ended
        assert!(!game.is_game_ended());
        assert!(!game.is_bingo_reached());
        assert!(!game.is_pouch_empty());

        // Test dump_if_ended for a non-ended game
        let dump_result = game.dump_if_ended();
        assert!(dump_result.is_err());
        assert!(dump_result.unwrap_err().contains("Game has not ended yet"));
    }

    #[test]
    fn test_selective_dump_logic() {
        let game = Game::new();

        // Test scenarios for selective dumping on newgame:

        // Scenario 1: Game not started - should not dump
        assert!(!game.has_game_started());
        assert!(!game.is_bingo_reached());
        // Logic: !game.has_game_started() -> no dump

        // Scenario 2: Game started but no BINGO - should dump
        // (We can't easily simulate this without complex setup, but we can verify the logic)
        // Logic: game.has_game_started() && !game.is_bingo_reached() -> dump

        // Scenario 3: Game with BINGO reached - should not dump (already dumped)
        // Logic: game.has_game_started() && game.is_bingo_reached() -> no dump

        // For now, just verify the boolean logic conditions are accessible
        assert!(!game.has_game_started());
        assert!(!game.is_bingo_reached());
        assert!(!game.is_game_ended());
    }

    #[test]
    fn test_running_game_info() {
        let game = Game::new();

        // Test get_running_game_info method
        let (game_id, created_at_string, created_at_systemtime) = game.get_running_game_info();

        // Verify the returned values
        assert!(!game_id.is_empty());
        assert!(game_id.starts_with("game_"));
        assert_eq!(game_id.len(), 13); // "game_" + 8 hex chars
        assert_eq!(game_id, game.id());

        // Verify creation time consistency
        assert!(!created_at_string.is_empty());
        assert!(created_at_string.contains("UTC"));
        assert_eq!(created_at_string, game.created_at_string());
        assert_eq!(created_at_systemtime, game.created_at());

        // Verify the SystemTime is recent (within last few seconds)
        let now = SystemTime::now();
        let time_diff = now.duration_since(created_at_systemtime).unwrap_or_default();
        assert!(time_diff.as_secs() < 5); // Should be created within last 5 seconds
    }

    #[test]
    fn test_game_registry_creation() {
        let registry = GameRegistry::new();

        // Test initial state
        assert_eq!(registry.total_games().unwrap(), 0);

        let games_list = registry.games_list().unwrap();
        assert!(games_list.is_empty());

        let status_summary = registry.status_summary().unwrap();
        assert_eq!(status_summary, (0, 0, 0)); // (new, active, closed)
    }

    #[test]
    fn test_game_registry_add_game() {
        let registry = GameRegistry::new();
        let game1 = Arc::new(Game::new());
        let game2 = Arc::new(Game::new());

        // Add first game
        let game1_id = registry.add_game(game1.clone()).unwrap();
        assert_eq!(game1_id, game1.id());
        assert_eq!(registry.total_games().unwrap(), 1);

        // Add second game
        let game2_id = registry.add_game(game2.clone()).unwrap();
        assert_eq!(game2_id, game2.id());
        assert_eq!(registry.total_games().unwrap(), 2);

        // Try to add the same game again (should fail)
        let duplicate_result = registry.add_game(game1.clone());
        assert!(duplicate_result.is_err());
        assert!(duplicate_result.unwrap_err().contains("already exists"));
        assert_eq!(registry.total_games().unwrap(), 2); // Count shouldn't change
    }

    #[test]
    fn test_game_registry_games_list() {
        let registry = GameRegistry::new();
        let game1 = Arc::new(Game::new());
        let game2 = Arc::new(Game::new());

        // Add games
        registry.add_game(game1.clone()).unwrap();
        registry.add_game(game2.clone()).unwrap();

        // Get games list
        let games_list = registry.games_list().unwrap();
        assert_eq!(games_list.len(), 2);

        // Check that both games are present with New status
        let game_ids: Vec<String> = games_list.iter().map(|(id, _, _)| id.clone()).collect();
        assert!(game_ids.contains(&game1.id()));
        assert!(game_ids.contains(&game2.id()));

        // All games should have New status initially
        for (_, status, _) in &games_list {
            assert_eq!(*status, GameStatus::New);
        }
    }

    #[test]
    fn test_game_registry_get_game() {
        let registry = GameRegistry::new();
        let game = Arc::new(Game::new());
        let game_id = game.id();

        // Get non-existent game
        let result = registry.get_game("non_existent_id").unwrap();
        assert!(result.is_none());

        // Add game and retrieve it
        registry.add_game(game.clone()).unwrap();
        let retrieved_game = registry.get_game(&game_id).unwrap();
        assert!(retrieved_game.is_some());

        let retrieved = retrieved_game.unwrap();
        assert_eq!(retrieved.id(), game_id);
    }

    #[test]
    fn test_game_registry_remove_game() {
        let registry = GameRegistry::new();
        let game = Arc::new(Game::new());
        let game_id = game.id();

        // Try to remove non-existent game
        let removed = registry.remove_game("non_existent_id").unwrap();
        assert!(!removed);

        // Add game and remove it
        registry.add_game(game.clone()).unwrap();
        assert_eq!(registry.total_games().unwrap(), 1);

        let removed = registry.remove_game(&game_id).unwrap();
        assert!(removed);
        assert_eq!(registry.total_games().unwrap(), 0);

        // Try to remove again (should return false)
        let removed_again = registry.remove_game(&game_id).unwrap();
        assert!(!removed_again);
    }

    #[test]
    fn test_game_registry_games_by_status() {
        let registry = GameRegistry::new();
        let game1 = Arc::new(Game::new());
        let game2 = Arc::new(Game::new());

        registry.add_game(game1.clone()).unwrap();
        registry.add_game(game2.clone()).unwrap();

        // Initially all games should be New
        let new_games = registry.games_by_status(GameStatus::New).unwrap();
        assert_eq!(new_games.len(), 2);
        assert!(new_games.contains(&game1.id()));
        assert!(new_games.contains(&game2.id()));

        let active_games = registry.games_by_status(GameStatus::Active).unwrap();
        assert!(active_games.is_empty());

        let closed_games = registry.games_by_status(GameStatus::Closed).unwrap();
        assert!(closed_games.is_empty());
    }

    #[test]
    fn test_game_registry_status_summary() {
        let registry = GameRegistry::new();
        let game1 = Arc::new(Game::new());
        let game2 = Arc::new(Game::new());

        // Empty registry
        let summary = registry.status_summary().unwrap();
        assert_eq!(summary, (0, 0, 0));

        // Add games
        registry.add_game(game1.clone()).unwrap();
        registry.add_game(game2.clone()).unwrap();

        // All should be New
        let summary = registry.status_summary().unwrap();
        assert_eq!(summary, (2, 0, 0)); // (new, active, closed)
    }

    #[test]
    fn test_game_registry_clear() {
        let registry = GameRegistry::new();
        let game1 = Arc::new(Game::new());
        let game2 = Arc::new(Game::new());

        registry.add_game(game1.clone()).unwrap();
        registry.add_game(game2.clone()).unwrap();
        assert_eq!(registry.total_games().unwrap(), 2);

        // Clear all games
        let cleared_count = registry.clear().unwrap();
        assert_eq!(cleared_count, 2);
        assert_eq!(registry.total_games().unwrap(), 0);

        // Clear empty registry
        let cleared_count_empty = registry.clear().unwrap();
        assert_eq!(cleared_count_empty, 0);
    }

    #[test]
    fn test_game_entry() {
        let game = Arc::new(Game::new());
        let game_id = game.id();
        let mut entry = GameEntry::new(game_id.clone(), game.clone());

        // Test initial values
        assert_eq!(entry.game_id, game_id);
        assert_eq!(entry.game.id(), game_id);
        assert_eq!(entry.status(), GameStatus::New);
        assert!(entry.closed_at.is_none());
        assert!(!entry.is_closed());

        // Test info string
        let info = entry.info();
        assert!(info.contains(&game_id));
        assert!(info.contains("New"));
        assert!(info.contains("board_len=0"));
        assert!(info.contains("score=0"));
        assert!(!info.contains("closed_at=")); // Should not have closed_at info for new game

        // Test registered_at_string
        let reg_time = entry.registered_at_string();
        assert!(reg_time.contains("UTC"));
        assert!(!reg_time.is_empty());

        // Test closed_at_string when None
        assert!(entry.closed_at_string().is_none());

        // Test status_with_update for a new game
        let status = entry.status_with_update();
        assert_eq!(status, GameStatus::New);
        assert!(entry.closed_at.is_none()); // Should still be None for non-closed game
    }

    #[test]
    fn test_game_status_conversions() {
        assert_eq!(GameStatus::New.as_str(), "New");
        assert_eq!(GameStatus::Active.as_str(), "Active");
        assert_eq!(GameStatus::Closed.as_str(), "Closed");

        // Test PartialEq
        assert_eq!(GameStatus::New, GameStatus::New);
        assert_ne!(GameStatus::New, GameStatus::Active);
        assert_ne!(GameStatus::Active, GameStatus::Closed);
    }

    #[test]
    fn test_game_entry_closed_at() {
        let game = Arc::new(Game::new());
        let game_id = game.id();
        let mut entry = GameEntry::new(game_id.clone(), game.clone());

        // Initially closed_at should be None
        assert!(entry.closed_at.is_none());
        assert!(entry.closed_at_string().is_none());
        assert!(!entry.is_closed());

        // Simulate game reaching BINGO by manually setting the scorecard
        {
            let mut scorecard = game.scorecard().lock().unwrap();
            scorecard.published_score = 15; // BINGO reached
        }

        // Now the game should be closed
        assert!(entry.is_closed());
        assert_eq!(entry.status(), GameStatus::Closed);

        // But closed_at should still be None until we call status_with_update
        assert!(entry.closed_at.is_none());

        // Call status_with_update to set the closed_at timestamp
        let status = entry.status_with_update();
        assert_eq!(status, GameStatus::Closed);
        assert!(entry.closed_at.is_some());

        // Test that closed_at_string returns a valid time string
        let closed_time_str = entry.closed_at_string();
        assert!(closed_time_str.is_some());
        let time_str = closed_time_str.unwrap();
        assert!(time_str.contains("UTC"));
        assert!(!time_str.is_empty());

        // Test that info now includes closed_at
        let info = entry.info();
        assert!(info.contains("Closed"));
        assert!(info.contains("closed_at="));

        // Test that update_closed_at doesn't change the timestamp once set
        let original_closed_at = entry.closed_at;
        entry.update_closed_at();
        assert_eq!(entry.closed_at, original_closed_at);
    }

    #[test]
    fn test_game_registry_clone() {
        let registry1 = GameRegistry::new();
        let game = Arc::new(Game::new());

        registry1.add_game(game.clone()).unwrap();
        assert_eq!(registry1.total_games().unwrap(), 1);

        // Clone the registry
        let registry2 = registry1.clone();

        // Both registries should reference the same data
        assert_eq!(registry2.total_games().unwrap(), 1);

        // Adding to one should affect the other (shared data)
        let new_game = Arc::new(Game::new());
        registry2.add_game(new_game.clone()).unwrap();

        assert_eq!(registry1.total_games().unwrap(), 2);
        assert_eq!(registry2.total_games().unwrap(), 2);
    }

    #[test]
    fn test_game_lifecycle_end_to_end() {
        // ========================================================================
        // PHASE 1: Create a new game and register it
        // ========================================================================

        let registry = GameRegistry::new();
        let game = Arc::new(Game::new());
        let game_id = game.id();

        // Verify initial game state
        assert!(!game.has_game_started());
        assert!(!game.is_bingo_reached());
        assert!(!game.is_game_ended());
        assert_eq!(game.board_length(), 0);
        assert_eq!(game.published_score(), 0);
        assert_eq!(game.pouch_length(), 90);

        // Add game to registry
        let registered_id = registry.add_game(game.clone()).unwrap();
        assert_eq!(registered_id, game_id);

        // ========================================================================
        // PHASE 2: Verify initial registry state and game entry
        // ========================================================================

        // Check registry statistics
        assert_eq!(registry.total_games().unwrap(), 1);
        let (new_count, active_count, closed_count) = registry.status_summary().unwrap();
        assert_eq!((new_count, active_count, closed_count), (1, 0, 0));

        // Check games list
        let games_list = registry.games_list().unwrap();
        assert_eq!(games_list.len(), 1);
        let (list_game_id, status, info) = &games_list[0];
        assert_eq!(*list_game_id, game_id);
        assert_eq!(*status, GameStatus::New);
        assert!(info.contains("New"));
        assert!(info.contains("board_len=0"));
        assert!(info.contains("score=0"));
        assert!(!info.contains("closed_at=")); // No closed_at for new game

        // Check games by status
        let new_games = registry.games_by_status(GameStatus::New).unwrap();
        assert_eq!(new_games.len(), 1);
        assert!(new_games.contains(&game_id));

        let active_games = registry.games_by_status(GameStatus::Active).unwrap();
        assert!(active_games.is_empty());

        let closed_games = registry.games_by_status(GameStatus::Closed).unwrap();
        assert!(closed_games.is_empty());

        // ========================================================================
        // PHASE 3: Extract some numbers to make the game active
        // ========================================================================

        println!("Extracting numbers to activate the game...");

        // Extract 5 numbers to get the game started and build some score
        for i in 1..=5 {
            let current_score = game.published_score();
            let extraction_result = game.extract_number(current_score, None);
            assert!(extraction_result.is_ok(), "Failed to extract number {i}: {extraction_result:?}");

            let (extracted_number, new_score) = extraction_result.unwrap();
            println!("Extracted number: {extracted_number}, score: {current_score} -> {new_score}");

            // Verify game state is progressing
            assert!((1..=90).contains(&extracted_number));
            assert!(new_score >= current_score); // Score should not decrease
            assert_eq!(game.board_length(), i as usize);
            assert_eq!(game.published_score(), new_score);
            assert_eq!(game.pouch_length(), 90 - i as usize);
        }

        // Verify game is now active
        assert!(game.has_game_started());
        assert!(!game.is_bingo_reached());
        assert!(!game.is_game_ended());

        // ========================================================================
        // PHASE 4: Verify registry state after game becomes active
        // ========================================================================

        let active_score = game.published_score();

        // Check updated registry statistics
        let (new_count, active_count, closed_count) = registry.status_summary().unwrap();
        assert_eq!((new_count, active_count, closed_count), (0, 1, 0));

        // Check updated games list
        let games_list = registry.games_list().unwrap();
        assert_eq!(games_list.len(), 1);
        let (list_game_id, status, info) = &games_list[0];
        assert_eq!(*list_game_id, game_id);
        assert_eq!(*status, GameStatus::Active);
        assert!(info.contains("Active"));
        assert!(info.contains("board_len=5"));
        assert!(info.contains(&format!("score={active_score}")));
        assert!(!info.contains("closed_at=")); // No closed_at for active game

        // Check games by status
        let new_games = registry.games_by_status(GameStatus::New).unwrap();
        assert!(new_games.is_empty());

        let active_games = registry.games_by_status(GameStatus::Active).unwrap();
        assert_eq!(active_games.len(), 1);
        assert!(active_games.contains(&game_id));

        let closed_games = registry.games_by_status(GameStatus::Closed).unwrap();
        assert!(closed_games.is_empty());

        // ========================================================================
        // PHASE 5: Extract numbers until BINGO is reached
        // ========================================================================

        println!("Extracting numbers until BINGO is reached...");

        // Continue extracting until we reach BINGO (score >= 15)
        let mut extractions = 5;
        while !game.is_bingo_reached() && extractions < 90 {
            let current_score = game.published_score();
            let extraction_result = game.extract_number(current_score, None);

            if extraction_result.is_err() {
                println!("Extraction failed at score {current_score}: {extraction_result:?}");
                break;
            }

            let (extracted_number, new_score) = extraction_result.unwrap();
            extractions += 1;

            println!("Extraction {extractions}: number={extracted_number}, score={new_score}");

            // Verify extraction validity
            assert!((1..=90).contains(&extracted_number));
            assert!(new_score >= current_score); // Score should not decrease
        }

        // Verify BINGO state
        assert!(game.is_bingo_reached(), "BINGO should have been reached");
        assert!(game.published_score() >= 15, "Score should be >= 15 for BINGO");
        assert!(game.is_game_ended(), "Game should be ended when BINGO is reached");
        assert!(game.has_game_started(), "Game should still show as started");

        println!("BINGO reached! Final score: {}, extractions: {}", game.published_score(), extractions);

        // ========================================================================
        // PHASE 6: Verify final registry state with closed game and closed_at
        // ========================================================================

        // Check final registry statistics
        let (new_count, active_count, closed_count) = registry.status_summary().unwrap();
        assert_eq!((new_count, active_count, closed_count), (0, 0, 1));

        // Check final games list with closed_at information
        let games_list = registry.games_list().unwrap();
        assert_eq!(games_list.len(), 1);
        let (list_game_id, status, info) = &games_list[0];
        assert_eq!(*list_game_id, game_id);
        assert_eq!(*status, GameStatus::Closed);
        assert!(info.contains("Closed"));
        assert!(info.contains(&format!("board_len={extractions}")));
        assert!(info.contains(&format!("score={}", game.published_score())));
        assert!(info.contains("closed_at=")); // Should now have closed_at

        // Check games by status - should all be in closed
        let new_games = registry.games_by_status(GameStatus::New).unwrap();
        assert!(new_games.is_empty());

        let active_games = registry.games_by_status(GameStatus::Active).unwrap();
        assert!(active_games.is_empty());

        let closed_games = registry.games_by_status(GameStatus::Closed).unwrap();
        assert_eq!(closed_games.len(), 1);
        assert!(closed_games.contains(&game_id));

        // ========================================================================
        // PHASE 7: Verify GameEntry closed_at timestamp is properly set
        // ========================================================================

        // Get the game entry directly to check closed_at
        let retrieved_game = registry.get_game(&game_id).unwrap();
        assert!(retrieved_game.is_some());

        // Access the GameEntry to verify closed_at (we need to access the registry's internal data)
        // Since we can't directly access the GameEntry, we'll verify through the info string
        let final_games_list = registry.games_list().unwrap();
        let (_, _, final_info) = &final_games_list[0];

        // Verify the info contains a properly formatted closed_at timestamp
        assert!(final_info.contains("closed_at="));
        let closed_at_part = final_info.split("closed_at=").nth(1).unwrap();
        let closed_at_time = closed_at_part.split(']').next().unwrap();
        assert!(closed_at_time.contains("UTC"));
        assert!(closed_at_time.len() > 10); // Should be a reasonable timestamp

        // ========================================================================
        // PHASE 8: Verify game state dump functionality
        // ========================================================================

        // Test that the game can be dumped since it has ended
        let dump_result = game.dump_if_ended();
        assert!(dump_result.is_ok(), "Should be able to dump ended game: {dump_result:?}");

        let dump_message = dump_result.unwrap();
        assert!(dump_message.contains("Game dumped to:"));
        assert!(dump_message.contains(&game_id));
        assert!(dump_message.contains(".json"));

        println!("Game successfully dumped: {dump_message}");

        // ========================================================================
        // PHASE 9: Final verification summary
        // ========================================================================

        println!("\n========== END-TO-END TEST SUMMARY ==========");
        println!("✓ Game created and registered successfully");
        println!("✓ Initial state: New game with no extractions");
        println!("✓ Registry correctly tracked New status");
        println!("✓ Game became Active after first extractions");
        println!("✓ Registry correctly tracked Active status");
        println!("✓ Game reached BINGO (Closed) after {extractions} extractions");
        println!("✓ Registry correctly tracked Closed status");
        println!("✓ GameEntry closed_at timestamp was properly set");
        println!("✓ Game state was successfully dumped to JSON");
        println!("✓ Final score: {}", game.published_score());
        println!("✓ Final board length: {}", game.board_length());
        println!("✓ Remaining numbers in pouch: {}", game.pouch_length());
        println!("==============================================\n");

        // Final assertions to ensure everything is in the expected state
        assert_eq!(registry.total_games().unwrap(), 1);
        assert!(game.is_game_ended());
        assert!(game.is_bingo_reached());
        assert!(game.published_score() >= 15);
        assert_eq!(game.board_length(), extractions as usize);
        assert_eq!(game.pouch_length(), 90 - extractions as usize);
    }
}
