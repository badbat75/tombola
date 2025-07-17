use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::Hasher;
use serde::{Deserialize, Serialize};

// Client registration structures
#[derive(Debug, Deserialize)]
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
    pub fn new(name: String, client_type: String) -> Self {
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
        if client_id == "0000000000000000" {
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
