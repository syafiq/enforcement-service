//! Error types for the enforcement service

use thiserror::Error;

#[derive(Debug, Error)]
pub enum EnforcementError {
    #[error("Policy error: {0}")]
    Policy(String),
    
    #[error("Entity not found: {0}")]
    EntityNotFound(String),
    
    #[error("Capability denied: {entity} cannot access {capability}")]
    CapabilityDenied {
        entity: String,
        capability: String,
    },
    
    #[error("Rate limit exceeded for {entity}: {message}")]
    RateLimitExceeded {
        entity: String,
        message: String,
    },
    
    #[error("Quota exceeded for {entity}: {message}")]
    QuotaExceeded {
        entity: String,
        message: String,
    },
    
    #[error("Session error: {0}")]
    Session(String),
    
    #[error("Configuration error: {0}")]
    Config(String),
    
    #[error("HAL error: {0}")]
    Hal(String),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("YAML error: {0}")]
    Yaml(#[from] serde_yaml::Error),
    
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, EnforcementError>;
