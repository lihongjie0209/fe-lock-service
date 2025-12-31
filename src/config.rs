use serde::Deserialize;
use std::env;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub storage_type: StorageType,
    pub redis_url: Option<String>,
    pub redis_username: Option<String>,
    pub redis_password: Option<String>,
    pub redis_db: Option<i64>,
    pub server_host: String,
    pub server_port: u16,
    pub memory_persist_enabled: bool,
    pub memory_persist_path: String,
    pub memory_persist_interval: u64, // 秒
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum StorageType {
    Memory,
    Redis,
}

impl Config {
    pub fn from_env() -> Self {
        let storage_type = env::var("STORAGE_TYPE")
            .unwrap_or_else(|_| "memory".to_string())
            .to_lowercase();

        let storage_type = match storage_type.as_str() {
            "redis" => StorageType::Redis,
            _ => StorageType::Memory,
        };

        let redis_url = if storage_type == StorageType::Redis {
            Some(
                env::var("REDIS_URL")
                    .unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string()),
            )
        } else {
            None
        };

        let redis_username = env::var("REDIS_USERNAME").ok();
        let redis_password = env::var("REDIS_PASSWORD").ok();
        let redis_db = env::var("REDIS_DB")
            .ok()
            .and_then(|s| s.parse::<i64>().ok());

        let server_host = env::var("SERVER_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
        let server_port = env::var("SERVER_PORT")
            .unwrap_or_else(|_| "8080".to_string())
            .parse()
            .unwrap_or(8080);

        let memory_persist_enabled = env::var("MEMORY_PERSIST_ENABLED")
            .unwrap_or_else(|_| "true".to_string())
            .parse()
            .unwrap_or(true);

        let memory_persist_path = env::var("MEMORY_PERSIST_PATH")
            .unwrap_or_else(|_| "./data/locks.json".to_string());

        let memory_persist_interval = env::var("MEMORY_PERSIST_INTERVAL")
            .unwrap_or_else(|_| "30".to_string())
            .parse()
            .unwrap_or(30);

        Self {
            storage_type,
            redis_url,
            redis_username,
            redis_password,
            redis_db,
            server_host,
            server_port,
            memory_persist_enabled,
            memory_persist_path,
            memory_persist_interval,
        }
    }
}
