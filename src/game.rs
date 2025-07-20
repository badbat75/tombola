// src/game.rs
// This module provides a unified Game struct that encapsulates all game state components
// and provides coordinated access to prevent deadlocks and simplify state management.

use std::sync::{Arc, Mutex};
use std::time::SystemTime;
use rand::Rng;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use crate::board::Board;
use crate::pouch::Pouch;
use crate::score::ScoreCard;
use crate::client::ClientRegistry;
use crate::card::CardAssignmentManager;
use crate::defs::Number;
use crate::extraction::perform_extraction;

/// Game struct that holds all shared game state components
/// This provides a single point of access for all game operations
/// and ensures proper mutex coordination to prevent deadlocks.
#[derive(Clone)]
pub struct Game {
    id: Arc<Mutex<String>>,
    created_at: Arc<Mutex<SystemTime>>,
    board: Arc<Mutex<Board>>,
    pouch: Arc<Mutex<Pouch>>,
    scorecard: Arc<Mutex<ScoreCard>>,
    client_registry: Arc<Mutex<ClientRegistry>>,
    card_manager: Arc<Mutex<CardAssignmentManager>>,
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
            client_registry: Arc::new(Mutex::new(ClientRegistry::new())),
            card_manager: Arc::new(Mutex::new(CardAssignmentManager::new())),
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

    /// Get a reference to the client registry Arc<Mutex<ClientRegistry>>
    pub fn client_registry(&self) -> &Arc<Mutex<ClientRegistry>> {
        &self.client_registry
    }

    /// Get a reference to the card manager Arc<Mutex<CardAssignmentManager>>
    pub fn card_manager(&self) -> &Arc<Mutex<CardAssignmentManager>> {
        &self.card_manager
    }

    /// Reset all game state to start a new game
    /// This follows the proper mutex acquisition order to prevent deadlocks
    pub fn reset_game(&self) -> Result<Vec<String>, Vec<String>> {
        let mut reset_components = Vec::new();
        let mut errors = Vec::new();

        // Generate new game ID and creation time for the fresh game
        let mut rng = rand::rng();
        let new_id = format!("game_{:08x}", rng.random::<u32>());
        
        // Update game ID and creation time
        if let Ok(mut id_lock) = self.id.lock() {
            *id_lock = new_id.clone();
        } else {
            errors.push("Failed to lock game ID for reset".to_string());
        }
        
        if let Ok(mut created_at_lock) = self.created_at.lock() {
            *created_at_lock = SystemTime::now();
        } else {
            errors.push("Failed to lock creation time for reset".to_string());
        }
        
        reset_components.push(format!("New game ID generated: {new_id}"));

        // Reset all game structures in coordinated order to prevent deadlocks
        // Follow the mutex acquisition order: pouch -> board -> scorecard -> card_manager -> client_registry

        // Reset Pouch (refill with numbers 1-90)
        if let Ok(mut pouch) = self.pouch.lock() {
            *pouch = Pouch::new();
            reset_components.push("Pouch refilled with numbers 1-90".to_string());
        } else {
            errors.push("Failed to lock pouch for reset".to_string());
        }

        // Reset Board (clear extracted numbers and marked positions)
        if let Ok(mut board) = self.board.lock() {
            *board = Board::new();
            reset_components.push("Board state cleared".to_string());
        } else {
            errors.push("Failed to lock board for reset".to_string());
        }

        // Reset ScoreCard (reset published score and score map)
        if let Ok(mut scorecard) = self.scorecard.lock() {
            *scorecard = ScoreCard::new();
            reset_components.push("Score card reset".to_string());
        } else {
            errors.push("Failed to lock scorecard for reset".to_string());
        }

        // Reset CardAssignmentManager (clear all card assignments)
        if let Ok(mut card_mgr) = self.card_manager.lock() {
            *card_mgr = CardAssignmentManager::new();
            reset_components.push("Card assignments cleared".to_string());
        } else {
            errors.push("Failed to lock card manager for reset".to_string());
        }

        // Reset ClientRegistry (clear all registered clients)
        if let Ok(mut registry) = self.client_registry.lock() {
            *registry = ClientRegistry::new();
            reset_components.push("Client registry cleared".to_string());
        } else {
            errors.push("Failed to lock client registry for reset".to_string());
        }

        if errors.is_empty() {
            Ok(reset_components)
        } else {
            Err(errors)
        }
    }

    /// Perform a number extraction using the coordinated extraction logic
    /// This encapsulates the complex mutex coordination required for extraction
    pub fn extract_number(&self, current_working_score: Number) -> Result<(Number, Number), String> {
        perform_extraction(
            &self.pouch,
            &self.board,
            &self.scorecard,
            &self.card_manager,
            current_working_score,
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

        let client_registry = {
            let guard = self.client_registry.lock()
                .map_err(|_| "Failed to lock client registry")?;
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
            client_registry,
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
    pub client_registry: ClientRegistry,
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
    fn test_game_reset() {
        let game = Game::new();
        let original_id = game.id();
        
        // Test that reset works properly and generates new game ID
        let result = game.reset_game();
        assert!(result.is_ok());
        
        let reset_components = result.unwrap();
        assert!(reset_components.contains(&"Pouch refilled with numbers 1-90".to_string()));
        assert!(reset_components.contains(&"Board state cleared".to_string()));
        assert!(reset_components.contains(&"Score card reset".to_string()));
        assert!(reset_components.contains(&"Card assignments cleared".to_string()));
        assert!(reset_components.contains(&"Client registry cleared".to_string()));
        
        // Verify that a new game ID was generated
        assert_ne!(game.id(), original_id);
        assert!(game.id().starts_with("game_"));
        assert!(reset_components.iter().any(|s| s.starts_with("New game ID generated:")));
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
}
