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

// Global client registry (keyed by client name)
pub type ClientRegistry = Arc<Mutex<HashMap<String, ClientInfo>>>;

// Client ID generation function
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
