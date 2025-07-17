use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone)]
pub struct ClientConfig {
    pub host: String,
    pub port: u16,
    pub timeout: u64,
    pub retry_attempts: u32,
    pub client_name: String,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 3000,
        }
    }
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 3000,
            timeout: 30,
            retry_attempts: 3,
            client_name: "DefaultClient".to_string(),
        }
    }
}

impl ClientConfig {
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        let content = fs::read_to_string(path)?;
        let config_map = parse_config(&content)?;
        
        let host = config_map.get("host")
            .unwrap_or(&"127.0.0.1".to_string())
            .clone();
        
        let port = config_map.get("port")
            .and_then(|p| p.parse::<u16>().ok())
            .unwrap_or(3000);
        
        let timeout = config_map.get("timeout")
            .and_then(|t| t.parse::<u64>().ok())
            .unwrap_or(30);
        
        let retry_attempts = config_map.get("retry_attempts")
            .and_then(|r| r.parse::<u32>().ok())
            .unwrap_or(3);
        
        let client_name = config_map.get("client_name")
            .unwrap_or(&"DefaultClient".to_string())
            .clone();
        
        Ok(ClientConfig { host, port, timeout, retry_attempts, client_name })
    }
    
    pub fn load_or_default() -> Self {
        let config_path = "conf/client.conf";
        
        match Self::from_file(config_path) {
            Ok(config) => {
                println!("📄 Loaded client configuration from {}", config_path);
                config
            }
            Err(e) => {
                println!("⚠️  Could not load client config from {}: {}. Using defaults.", config_path, e);
                Self::default()
            }
        }
    }
    
    pub fn server_url(&self) -> String {
        format!("http://{}:{}", self.host, self.port)
    }
}

impl ServerConfig {
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        let content = fs::read_to_string(path)?;
        let config_map = parse_config(&content)?;
        
        let host = config_map.get("host")
            .unwrap_or(&"127.0.0.1".to_string())
            .clone();
        
        let port = config_map.get("port")
            .and_then(|p| p.parse::<u16>().ok())
            .unwrap_or(3000);
        
        Ok(ServerConfig { host, port })
    }
    
    pub fn load_or_default() -> Self {
        let config_path = "conf/server.conf";
        
        match Self::from_file(config_path) {
            Ok(config) => {
                println!("📄 Loaded configuration from {}", config_path);
                config
            }
            Err(e) => {
                println!("⚠️  Could not load config from {}: {}. Using defaults.", config_path, e);
                Self::default()
            }
        }
    }
}

fn parse_config(content: &str) -> Result<HashMap<String, String>, Box<dyn std::error::Error>> {
    let mut config = HashMap::new();
    
    for line in content.lines() {
        let line = line.trim();
        
        // Skip empty lines and comments
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        
        // Parse key = value pairs
        if let Some((key, value)) = line.split_once('=') {
            let key = key.trim().to_string();
            let value = value.trim().to_string();
            config.insert(key, value);
        }
    }
    
    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_parse_config() {
        let content = r#"
            # This is a comment
            host = 192.168.1.100
            port = 8080
            # Another comment
            max_connections = 50
        "#;
        
        let config = parse_config(content).unwrap();
        assert_eq!(config.get("host"), Some(&"192.168.1.100".to_string()));
        assert_eq!(config.get("port"), Some(&"8080".to_string()));
        assert_eq!(config.get("max_connections"), Some(&"50".to_string()));
    }
    
    #[test]
    fn test_server_config_default() {
        let config = ServerConfig::default();
        assert_eq!(config.host, "127.0.0.1");
        assert_eq!(config.port, 3000);
    }
    
    #[test]
    fn test_client_config_default() {
        let config = ClientConfig::default();
        assert_eq!(config.host, "127.0.0.1");
        assert_eq!(config.port, 3000);
        assert_eq!(config.timeout, 30);
        assert_eq!(config.retry_attempts, 3);
        assert_eq!(config.client_name, "DefaultClient");
    }
    
    #[test]
    fn test_client_config_server_url() {
        let config = ClientConfig {
            host: "192.168.1.100".to_string(),
            port: 8080,
            timeout: 30,
            retry_attempts: 3,
            client_name: "TestClient".to_string(),
        };
        assert_eq!(config.server_url(), "http://192.168.1.100:8080");
    }
}
