//! Configuration management
//!
//! Loads configuration from environment variables with sensible defaults.

use serde::Deserialize;
use std::env;

/// Application configuration
#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    /// Server settings
    pub server: ServerConfig,
    /// Database connection settings
    pub database: DatabaseConfig,
    /// File storage settings
    pub storage: StorageConfig,
    /// Transcription provider settings
    pub transcription: TranscriptionConfig,
    /// Summary/LLM provider settings
    pub summary: SummaryProviderConfig,
    /// Authentication settings
    pub auth: AuthConfig,
}

/// Server configuration
#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    /// Host to bind to
    pub host: String,
    /// Port to listen on
    pub port: u16,
    /// Logging level (trace, debug, info, warn, error)
    pub log_level: String,
}

/// Database configuration
#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseConfig {
    /// PostgreSQL connection URL
    pub url: String,
    /// Maximum number of connections in pool
    pub max_connections: u32,
    /// Minimum number of idle connections
    pub min_connections: u32,
}

/// Storage configuration
#[derive(Debug, Clone, Deserialize)]
pub struct StorageConfig {
    /// Path to store recordings
    pub recordings_path: String,
    /// Maximum file size in MB
    pub max_file_size_mb: u64,
    /// Recording retention period in days
    pub retention_days: u32,
}

/// Transcription configuration
#[derive(Debug, Clone, Deserialize)]
pub struct TranscriptionConfig {
    /// Default transcription provider
    pub default_provider: String,
    /// Whisper model name (tiny, base, small, medium, large-v3)
    pub whisper_model: String,
    /// NVIDIA API key (optional)
    pub nvidia_api_key: Option<String>,
    /// NVIDIA base URL
    pub nvidia_base_url: String,
}

/// Summary provider configuration
#[derive(Debug, Clone, Deserialize)]
pub struct SummaryProviderConfig {
    /// Default summary provider
    pub default_provider: String,
    /// Ollama base URL
    pub ollama_base_url: String,
    /// Ollama model name
    pub ollama_model: String,
    /// OpenRouter API key (optional)
    pub openrouter_api_key: Option<String>,
    /// OpenRouter model name
    pub openrouter_model: Option<String>,
}

/// Authentication configuration
#[derive(Debug, Clone, Deserialize)]
pub struct AuthConfig {
    /// JWT signing secret
    pub jwt_secret: String,
    /// Access token expiry in hours
    pub token_expiry_hours: u64,
    /// Refresh token expiry in days
    pub refresh_token_expiry_days: u64,
}

impl AppConfig {
    /// Load configuration from environment variables
    ///
    /// Uses the `config` crate to merge:
    /// 1. Default values
    /// 2. Environment variables (MEETELY__* prefix)
    /// 3. Optional .env file
    pub fn from_env() -> Result<Self, config::ConfigError> {
        // Try to load .env file (optional, for development)
        let _ = dotenvy::dotenv();

        let config = config::Config::builder()
            // Default values
            .set_default("server.host", "0.0.0.0")?
            .set_default("server.port", 8080)?
            .set_default("server.log_level", "info")?
            .set_default("database.max_connections", 10)?
            .set_default("database.min_connections", 2)?
            .set_default("storage.max_file_size_mb", 1024)?
            .set_default("storage.retention_days", 30)?
            .set_default("transcription.default_provider", "whisper")?
            .set_default("transcription.whisper_model", "large-v3")?
            .set_default(
                "transcription.nvidia_base_url",
                "https://integrate.api.nvidia.com/v1",
            )?
            .set_default("summary.default_provider", "ollama")?
            .set_default("summary.ollama_model", "llama3.1:8b")?
            .set_default("summary.ollama_base_url", "http://localhost:11434")?
            .set_default("auth.token_expiry_hours", 24)?
            .set_default("auth.refresh_token_expiry_days", 7)?
            // Environment variables (MEETELY__ prefix, double underscore as separator)
            .add_source(config::Environment::with_prefix("MEETELY").separator("__"))
            .build()?;

        config.try_deserialize()
    }

    /// Get API base URL for the server
    pub fn server_url(&self) -> String {
        format!("http://{}:{}", self.server.host, self.server.port)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_config_from_default_env() {
        // Clear all MEETELY env vars for clean test
        let mut env_vars: HashMap<String, String> = HashMap::new();
        for (key, _) in env::vars() {
            if key.starts_with("MEETELY__") {
                env_vars.insert(key.clone(), env::var(&key).unwrap());
                env::remove_var(&key);
            }
        }

        // Load defaults
        let config = AppConfig::from_env().expect("Should load default config");

        // Verify defaults
        assert_eq!(config.server.host, "0.0.0.0");
        assert_eq!(config.server.port, 8080);
        assert_eq!(config.server.log_level, "info");
        assert_eq!(config.database.max_connections, 10);
        assert_eq!(config.database.min_connections, 2);
        assert_eq!(config.storage.max_file_size_mb, 1024);
        assert_eq!(config.storage.retention_days, 30);
        assert_eq!(config.transcription.default_provider, "whisper");
        assert_eq!(config.transcription.whisper_model, "large-v3");
        assert_eq!(config.summary.default_provider, "ollama");
        assert_eq!(config.summary.ollama_model, "llama3.1:8b");
        assert_eq!(config.auth.token_expiry_hours, 24);
        assert_eq!(config.auth.refresh_token_expiry_days, 7);

        // Restore env vars
        for (key, value) in env_vars {
            env::set_var(&key, &value);
        }
    }

    #[test]
    fn test_config_from_env() {
        // Set test environment variables
        env::set_var("MEETELY__SERVER__PORT", "9090");
        env::set_var("MEETELY__DATABASE__URL", "postgresql://test:test@localhost:5432/test");
        env::set_var("MEETELY__STORAGE__RECORDINGS_PATH", "/tmp/test_recordings");
        env::set_var("MEETELY__AUTH__JWT_SECRET", "test-secret-key");

        let config = AppConfig::from_env().expect("Should load config from env");

        // Verify custom values
        assert_eq!(config.server.port, 9090);
        assert_eq!(
            config.database.url,
            "postgresql://test:test@localhost:5432/test"
        );
        assert_eq!(config.storage.recordings_path, "/tmp/test_recordings");
        assert_eq!(config.auth.jwt_secret, "test-secret-key");

        // Cleanup
        env::remove_var("MEETELY__SERVER__PORT");
        env::remove_var("MEETELY__DATABASE__URL");
        env::remove_var("MEETELY__STORAGE__RECORDINGS_PATH");
        env::remove_var("MEETELY__AUTH__JWT_SECRET");
    }
}