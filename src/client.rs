use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::Hasher;
use std::sync::{Arc, Mutex};
use serde::{Deserialize, Serialize};

// Client registration structures
#[derive(Debug, Deserialize, Clone)]
pub struct RegisterRequest {
    pub name: String,
    pub client_type: String,
    pub nocard: Option<u32>,  // Number of cards to generate during registration
    pub email: Option<String>,  // Optional email for registration
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
    pub email: String,  // Internal field, not exposed through APIs
}

impl ClientInfo {
    pub fn new(name: &str, client_type: &str, email: &str) -> Self {
        // Generate a client ID based on name, type, and current time
        let mut hasher = DefaultHasher::new();
        
        // Include name, type, and current time for uniqueness
        hasher.write(name.as_bytes());
        hasher.write(client_type.as_bytes());
        
        // Use high-resolution timestamp for better uniqueness
        if let Ok(duration) = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
            hasher.write(&duration.as_nanos().to_le_bytes());
        }
        
        let hash = hasher.finish();
        let client_id = format!("{hash:016X}");

        ClientInfo {
            id: client_id,
            name: name.to_string(),
            client_type: client_type.to_string(),
            registered_at: std::time::SystemTime::now(),
            email: email.to_string(),
        }
    }

    pub fn client_id(&self) -> &str {
        &self.id
    }
}

// Unified client registry with internal thread safety (following GameRegistry model)
#[derive(Debug)]
pub struct ClientRegistry {
    clients: Arc<Mutex<HashMap<String, ClientInfo>>>,
}

impl Default for ClientRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ClientRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self {
            clients: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Insert a client into the registry (keyed by client ID)
    /// Returns the previous client if one existed with the same ID, or an error message
    pub fn insert(&self, client: ClientInfo) -> Result<Option<ClientInfo>, String> {
        let mut clients_lock = self.clients.lock()
            .map_err(|_| "Failed to lock client registry")?;
        
        Ok(clients_lock.insert(client.id.clone(), client))
    }

    /// Get a client by client name (for global registry lookup)
    pub fn get_by_name(&self, client_name: &str) -> Result<Option<ClientInfo>, String> {
        let clients_lock = self.clients.lock()
            .map_err(|_| "Failed to lock client registry")?;
        
        // Find client by name
        for client in clients_lock.values() {
            if client.name == client_name {
                return Ok(Some(client.clone()));
            }
        }
        
        Ok(None)
    }

    /// Get a client by client ID
    pub fn get(&self, client_id: &str) -> Result<Option<ClientInfo>, String> {
        let clients_lock = self.clients.lock()
            .map_err(|_| "Failed to lock client registry")?;
        
        Ok(clients_lock.get(client_id).cloned())
    }

    /// Get a client by client ID (alias for consistency)
    pub fn get_by_client_id(&self, client_id: &str) -> Result<Option<ClientInfo>, String> {
        self.get(client_id)
    }

    /// Get all clients as a vector (since we can't return iterator with lock)
    pub fn get_all_clients(&self) -> Result<Vec<ClientInfo>, String> {
        let clients_lock = self.clients.lock()
            .map_err(|_| "Failed to lock client registry")?;
        
        Ok(clients_lock.values().cloned().collect())
    }

    /// Check if registry is empty
    pub fn is_empty(&self) -> Result<bool, String> {
        let clients_lock = self.clients.lock()
            .map_err(|_| "Failed to lock client registry")?;
        
        Ok(clients_lock.is_empty())
    }

    /// Get number of clients
    pub fn len(&self) -> Result<usize, String> {
        let clients_lock = self.clients.lock()
            .map_err(|_| "Failed to lock client registry")?;
        
        Ok(clients_lock.len())
    }

    /// Check if a client exists by client ID
    pub fn contains_client(&self, client_id: &str) -> Result<bool, String> {
        let clients_lock = self.clients.lock()
            .map_err(|_| "Failed to lock client registry")?;
        
        Ok(clients_lock.contains_key(client_id))
    }

    /// Helper function to get client name by client ID
    pub fn get_client_name_by_id(&self, client_id: &str) -> Result<Option<String>, String> {
        if client_id == crate::board::BOARD_ID {
            return Ok(Some("Board".to_string()));
        }

        let clients_lock = self.clients.lock()
            .map_err(|_| "Failed to lock client registry")?;

        Ok(clients_lock.get(client_id).map(|client| client.name.clone()))
    }

    /// Helper function to get client info by client ID
    pub fn get_client_info_by_id(&self, client_id: &str) -> Result<Option<ClientInfo>, String> {
        self.get(client_id)
    }

    /// Remove a client by client ID
    pub fn remove(&self, client_id: &str) -> Result<Option<ClientInfo>, String> {
        let mut clients_lock = self.clients.lock()
            .map_err(|_| "Failed to lock client registry")?;
        
        Ok(clients_lock.remove(client_id))
    }

    /// Remove a client by client ID (alias for consistency)
    pub fn remove_by_client_id(&self, client_id: &str) -> Result<Option<ClientInfo>, String> {
        self.remove(client_id)
    }

    /// Clear all clients from the registry
    pub fn clear(&self) -> Result<usize, String> {
        let mut clients_lock = self.clients.lock()
            .map_err(|_| "Failed to lock client registry")?;

        let count = clients_lock.len();
        clients_lock.clear();
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_info_generation() {
        let client1 = ClientInfo::new("player1", "player", "player1@example.com");
        let client2 = ClientInfo::new("player2", "player", "player2@example.com");
        let client3 = ClientInfo::new("player1", "player", "player1@example.com"); // Same name, should get different ID

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
    }

    #[test]
    fn test_client_registry_operations() {
        let registry = ClientRegistry::new();
        let client = ClientInfo::new("testplayer", "player", "test@example.com");

        // Should be able to register
        let result = registry.insert(client.clone()).unwrap();
        assert!(result.is_none(), "First insert should return None");

        // Should return previous value when inserting same client ID
        let new_client = ClientInfo::new("newplayer", "player", "new@example.com");
        let updated_client = ClientInfo { 
            id: client.id.clone(), 
            name: new_client.name,
            client_type: new_client.client_type,
            registered_at: new_client.registered_at,
            email: String::new(),  // Default empty email for test
        };
        let result = registry.insert(updated_client).unwrap();
        assert!(result.is_some(), "Should return previous client when client ID exists");
    }

    #[test]
    fn test_client_registry_lookup() {
        let registry = ClientRegistry::new();
        let client = ClientInfo::new("testplayer", "player", "test@example.com");

        let _ = registry.insert(client.clone());

        // Test lookup by client ID
        let found = registry.get(&client.id).unwrap();
        assert!(found.is_some(), "Should find client by ID");
        assert_eq!(found.unwrap().name, "testplayer");

        // Test contains_client
        assert!(registry.contains_client(&client.id).unwrap(), "Should contain client");
        assert!(!registry.contains_client("nonexistent").unwrap(), "Should not contain nonexistent client");

        // Test get_client_name_by_id
        let name = registry.get_client_name_by_id(&client.id).unwrap();
        assert_eq!(name, Some("testplayer".to_string()));

        // Test board client special case
        let board_name = registry.get_client_name_by_id(crate::board::BOARD_ID).unwrap();
        assert_eq!(board_name, Some("Board".to_string()));
    }

    #[test]
    fn test_client_registry_collection_methods() {
        let registry = ClientRegistry::new();
        let client1 = ClientInfo::new("player1", "player", "player1@example.com");
        let client2 = ClientInfo::new("player2", "player", "player2@example.com");
        let client3 = ClientInfo::new("player3", "player", "player3@example.com");

        let _ = registry.insert(client1);
        let _ = registry.insert(client2);
        let _ = registry.insert(client3);

        // Test collection methods
        assert_eq!(registry.len().unwrap(), 3, "Registry should contain 3 clients");
        assert!(!registry.is_empty().unwrap(), "Registry should not be empty");

        let clients = registry.get_all_clients().unwrap();
        let names: Vec<String> = clients.iter().map(|c| c.name.clone()).collect();
        assert!(names.contains(&"player1".to_string()));
        assert!(names.contains(&"player2".to_string()));
        assert!(names.contains(&"player3".to_string()));
    }
}
