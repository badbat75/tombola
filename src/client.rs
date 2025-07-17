use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::Hasher;
use std::sync::{Arc, Mutex};
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
#[derive(Debug, Clone)]
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

    // Helper function to get client name by client ID
    pub fn get_client_name_by_id(client_id: &str, registry: &ClientRegistry) -> Option<String> {
        if client_id == "0000000000000000" {
            return Some("Board".to_string());
        }
        
        if let Ok(registry_guard) = registry.lock() {
            for client_info in registry_guard.values() {
                if client_info.id == client_id {
                    return Some(client_info.name.clone());
                }
            }
        }
        None
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
pub type ClientRegistry = Arc<Mutex<HashMap<String, ClientInfo>>>;
