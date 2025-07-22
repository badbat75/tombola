use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::Hasher;
use serde::{Deserialize, Serialize};

// Board client ID constant used throughout the application for client operations
pub const BOARDCLIENT_ID: &str = "0000000000000000";

/// Returns the board client's client ID as a String
#[inline]
pub fn boardclient_id() -> String {
    BOARDCLIENT_ID.to_string()
}

/// Returns the board client ID as a String (generic helper for any string conversion)
#[inline]
pub fn boardclient_id_string() -> String {
    BOARDCLIENT_ID.to_string()
}

// Client registration structures
#[derive(Debug, Deserialize, Clone)]
pub struct RegisterRequest {
    pub name: String,
    pub client_type: String,
    pub nocard: Option<u32>,  // Number of cards to generate during registration
}

#[derive(Debug, Serialize)]
pub struct RegisterResponse {
    pub client_id: String,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct ClientInfoResponse {
    pub client_id: String,
    pub name: String,
    pub client_type: String,
    pub registered_at: String,
}

// Client information storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientInfo {
    pub id: String,
    pub name: String,
    pub client_type: String,
    pub registered_at: std::time::SystemTime,
}

impl ClientInfo {
    // Generate a unique client ID based on name and client type
    pub fn generate_client_id(name: &str, client_type: &str) -> String {
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

    // Create a new ClientInfo with generated ID
    pub fn new(name: impl Into<String>, client_type: impl Into<String>) -> Self {
        let name = name.into();
        let client_type = client_type.into();
        let id = Self::generate_client_id(&name, &client_type);
        ClientInfo {
            id,
            name,
            client_type,
            registered_at: std::time::SystemTime::now(),
        }
    }
}

// Global client registry (keyed by client name)
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct ClientRegistry {
    clients: HashMap<String, ClientInfo>,
}

impl ClientRegistry {
    // Create a new empty registry
    pub fn new() -> Self {
        Self {
            clients: HashMap::new(),
        }
    }

    // Insert a client into the registry (only if no numbers have been extracted)
    pub fn insert(&mut self, key: String, client: ClientInfo, numbers_extracted: bool) -> Result<Option<ClientInfo>, String> {
        if numbers_extracted {
            return Err("Cannot register new clients after numbers have been extracted".to_string());
        }
        Ok(self.clients.insert(key, client))
    }

    // Get a client by key
    pub fn get(&self, key: &str) -> Option<&ClientInfo> {
        self.clients.get(key)
    }

    // Get all clients
    pub fn values(&self) -> std::collections::hash_map::Values<String, ClientInfo> {
        self.clients.values()
    }

    // Check if registry is empty
    pub fn is_empty(&self) -> bool {
        self.clients.is_empty()
    }

    // Get number of clients
    pub fn len(&self) -> usize {
        self.clients.len()
    }

    // Helper function to get client name by client ID
    pub fn get_client_name_by_id(&self, client_id: &str) -> Option<String> {
        if client_id == BOARDCLIENT_ID {
            return Some("Board".to_string());
        }

        for client_info in self.clients.values() {
            if client_info.id == client_id {
                return Some(client_info.name.to_string());
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_info_generation() {
        let client1 = ClientInfo::new("player1", "player");
        let client2 = ClientInfo::new("player2", "player");
        let client3 = ClientInfo::new("player1", "player"); // Same name, should get different ID

        // Each client should have a unique ID
        assert_ne!(client1.id, client2.id, "Different clients should have different IDs");
        assert_ne!(client1.id, client3.id, "Even same name should get different ID due to timestamp");

        // IDs should be 16 character hex strings
        assert_eq!(client1.id.len(), 16, "Client ID should be 16 characters");
        assert_eq!(client2.id.len(), 16, "Client ID should be 16 characters");

        // IDs should only contain hex characters
        assert!(client1.id.chars().all(|c| c.is_ascii_hexdigit()), "Client ID should only contain hex digits");
        assert!(client2.id.chars().all(|c| c.is_ascii_hexdigit()), "Client ID should only contain hex digits");

        // Names and types should be preserved
        assert_eq!(client1.name, "player1");
        assert_eq!(client1.client_type, "player");
        assert_eq!(client2.name, "player2");
        assert_eq!(client2.client_type, "player");
    }

    #[test]
    fn test_client_registry_basic_operations() {
        let mut registry = ClientRegistry::new();

        // Registry should start empty
        assert_eq!(registry.len(), 0);
        assert!(registry.is_empty());

        // Add a client
        let client = ClientInfo::new("testplayer", "player");
        let client_id = client.id.clone();
        let client_name = client.name.clone();

        let result = registry.insert(client_name.clone(), client, false);
        assert!(result.is_ok(), "Should be able to insert client when no numbers extracted");

        // Registry should now have one client
        assert_eq!(registry.len(), 1);
        assert!(!registry.is_empty());

        // Should be able to retrieve the client
        let retrieved_client = registry.get(&client_name);
        assert!(retrieved_client.is_some(), "Should be able to retrieve inserted client");
        assert_eq!(retrieved_client.unwrap().id, client_id);
        assert_eq!(retrieved_client.unwrap().name, client_name);
        assert_eq!(retrieved_client.unwrap().client_type, "player");
    }

    #[test]
    fn test_client_registry_registration_restrictions() {
        let mut registry = ClientRegistry::new();
        let client = ClientInfo::new("testplayer", "player");

        // Should be able to register when no numbers extracted
        let result = registry.insert("testplayer".to_string(), client.clone(), false);
        assert!(result.is_ok(), "Should allow registration when no numbers extracted");

        // Should not be able to register new clients after numbers extracted
        let new_client = ClientInfo::new("newplayer", "player");
        let result = registry.insert("newplayer".to_string(), new_client, true);
        assert!(result.is_err(), "Should not allow registration after numbers extracted");
        assert!(result.unwrap_err().contains("Cannot register new clients after numbers have been extracted"));
    }

    #[test]
    fn test_client_registry_get_client_name_by_id() {
        let mut registry = ClientRegistry::new();

        // Test board client ID
        let board_name = registry.get_client_name_by_id(BOARDCLIENT_ID);
        assert_eq!(board_name, Some("Board".to_string()), "Should return 'Board' for board client ID");

        // Add a regular client
        let client = ClientInfo::new("testplayer", "player");
        let client_id = client.id.clone();
        let _ = registry.insert("testplayer".to_string(), client, false);

        // Should be able to find client by ID
        let found_name = registry.get_client_name_by_id(&client_id);
        assert_eq!(found_name, Some("testplayer".to_string()), "Should find client name by ID");

        // Should return None for non-existent ID
        let not_found = registry.get_client_name_by_id("NONEXISTENT");
        assert_eq!(not_found, None, "Should return None for non-existent client ID");
    }

    #[test]
    fn test_global_client_id_consistency() {
        // Test that the same client name gets different IDs when created multiple times
        // (this simulates what should happen when a client registers to multiple games)

        let client1 = ClientInfo::new("sameplayer", "player");
        let client2 = ClientInfo::new("sameplayer", "player");

        // Different instances should have different IDs due to timestamp differences
        assert_ne!(client1.id, client2.id, "Different instances should have different IDs");

        // But both should have the same name and type
        assert_eq!(client1.name, client2.name);
        assert_eq!(client1.client_type, client2.client_type);

        // This demonstrates why we need a global registry to reuse client IDs
        // across games - without it, each registration would create a new ID
    }

    #[test]
    fn test_client_registry_multiple_clients() {
        let mut registry = ClientRegistry::new();

        // Add multiple clients
        let client1 = ClientInfo::new("player1", "player");
        let client2 = ClientInfo::new("player2", "observer");
        let client3 = ClientInfo::new("player3", "player");

        let client1_id = client1.id.clone();
        let client2_id = client2.id.clone();
        let client3_id = client3.id.clone();

        let _ = registry.insert("player1".to_string(), client1, false);
        let _ = registry.insert("player2".to_string(), client2, false);
        let _ = registry.insert("player3".to_string(), client3, false);

        assert_eq!(registry.len(), 3, "Should have 3 clients");

        // Test that we can find all clients by ID
        assert_eq!(registry.get_client_name_by_id(&client1_id), Some("player1".to_string()));
        assert_eq!(registry.get_client_name_by_id(&client2_id), Some("player2".to_string()));
        assert_eq!(registry.get_client_name_by_id(&client3_id), Some("player3".to_string()));

        // Test iteration over all clients
        let mut client_ids: Vec<String> = registry.values().map(|c| c.id.clone()).collect();
        client_ids.sort();

        let mut expected_ids = vec![client1_id, client2_id, client3_id];
        expected_ids.sort();

        assert_eq!(client_ids, expected_ids, "Should be able to iterate over all clients");
    }
}
