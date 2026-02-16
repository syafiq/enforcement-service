//! Enforcement service implementation

use crate::config::{PolicyConfig, EntityConfig};
use crate::error::{EnforcementError, Result};
use elastic_tee_hal::interfaces::{HalProvider, PlatformInterface, CryptoInterface, RandomInterface, ClockInterface};
use elastic_tee_hal::providers::*;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use chrono::{DateTime, Utc, Duration};
use uuid::Uuid;

/// Session information
#[derive(Debug, Clone)]
pub struct Session {
    pub session_id: Uuid,
    pub entity_id: String,
    pub granted_capabilities: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub operation_count: u64,
}

/// Audit event
#[derive(Debug, Clone)]
pub struct AuditEvent {
    pub timestamp: DateTime<Utc>,
    pub entity_id: String,
    pub session_id: Uuid,
    pub interface: String,
    pub operation: String,
    pub success: bool,
    pub error: Option<String>,
}

/// Main enforcement service
pub struct EnforcementService {
    policy: PolicyConfig,
    sessions: Arc<RwLock<HashMap<Uuid, Session>>>,
    audit_log: Arc<RwLock<Vec<AuditEvent>>>,
    operation_counters: Arc<RwLock<HashMap<String, OperationCounter>>>,
}

#[derive(Debug)]
struct OperationCounter {
    count: u64,
    last_reset: DateTime<Utc>,
    rate_limit: u64,
}

impl EnforcementService {
    /// Create a new enforcement service with the given policy
    pub fn new(policy: PolicyConfig) -> Result<Self> {
        policy.validate()?;
        
        Ok(Self {
            policy,
            sessions: Arc::new(RwLock::new(HashMap::new())),
            audit_log: Arc::new(RwLock::new(Vec::new())),
            operation_counters: Arc::new(RwLock::new(HashMap::new())),
        })
    }
    
    /// Load policy from file
    pub fn from_file(path: impl AsRef<std::path::Path>) -> Result<Self> {
        let policy = PolicyConfig::from_file(path)?;
        Self::new(policy)
    }
    
    /// Create a new session for an entity
    pub async fn create_session(&self, entity_id: &str) -> Result<Session> {
        // Find entity in policy
        let entity = self.policy.find_entity(entity_id)
            .ok_or_else(|| {
                if self.policy.settings.strict_mode {
                    EnforcementError::EntityNotFound(entity_id.to_string())
                } else {
                    // In non-strict mode, we could create a default restricted entity
                    EnforcementError::EntityNotFound(entity_id.to_string())
                }
            })?;
        
        let session_id = Uuid::new_v4();
        let now = Utc::now();
        let expires_at = now + Duration::hours(24); // 24 hour session
        
        let session = Session {
            session_id,
            entity_id: entity_id.to_string(),
            granted_capabilities: entity.capabilities.list_granted(),
            created_at: now,
            expires_at,
            operation_count: 0,
        };
        
        let mut sessions = self.sessions.write().await;
        sessions.insert(session_id, session.clone());
        
        log::info!(
            "Created session {} for entity {} with capabilities: {:?}",
            session_id,
            entity_id,
            session.granted_capabilities
        );
        
        Ok(session)
    }
    
    /// Get session information
    pub async fn get_session(&self, session_id: Uuid) -> Result<Session> {
        let sessions = self.sessions.read().await;
        let session = sessions.get(&session_id)
            .ok_or_else(|| EnforcementError::Session("Session not found".to_string()))?;
        
        // Check if expired
        if Utc::now() > session.expires_at {
            return Err(EnforcementError::Session("Session expired".to_string()));
        }
        
        Ok(session.clone())
    }
    
    /// Create a restricted HAL provider for an entity
    pub async fn create_restricted_hal(&self, entity_id: &str) -> Result<HalProvider> {
        let entity = self.policy.find_entity(entity_id)
            .ok_or_else(|| EnforcementError::EntityNotFound(entity_id.to_string()))?;
        
        let mut hal = HalProvider::new();
        
        // Platform interface
        if entity.capabilities.platform {
            match DefaultPlatformProvider::new() {
                Ok(platform) => {
                    hal.platform = Some(Box::new(platform));
                    log::debug!("Granted platform capability to {}", entity_id);
                }
                Err(e) => {
                    log::warn!("Failed to create platform provider for {}: {}", entity_id, e);
                }
            }
        }
        
        // Capabilities interface
        if entity.capabilities.capabilities {
            hal.capabilities = Some(Box::new(DefaultCapabilitiesProvider::new()));
            log::debug!("Granted capabilities capability to {}", entity_id);
        }
        
        // Crypto interface
        if entity.capabilities.crypto {
            hal.crypto = Some(Box::new(DefaultCryptoProvider::new()));
            log::debug!("Granted crypto capability to {}", entity_id);
        }
        
        // Random interface
        if entity.capabilities.random {
            hal.random = Some(Box::new(DefaultRandomProvider::new()));
            log::debug!("Granted random capability to {}", entity_id);
        }
        
        // Clock interface
        if entity.capabilities.clock {
            hal.clock = Some(Box::new(DefaultClockProvider::new()));
            log::debug!("Granted clock capability to {}", entity_id);
        }
        
        // Storage interface (when implemented)
        if entity.capabilities.storage {
            log::debug!("Storage capability requested for {} (not yet implemented)", entity_id);
        }
        
        Ok(hal)
    }
    
    /// Check if entity has a specific capability
    pub fn has_capability(&self, entity_id: &str, capability: &str) -> Result<bool> {
        let entity = self.policy.find_entity(entity_id)
            .ok_or_else(|| EnforcementError::EntityNotFound(entity_id.to_string()))?;
        
        Ok(entity.capabilities.has_capability(capability))
    }
    
    /// Check rate limit for an entity
    pub async fn check_rate_limit(&self, entity_id: &str, interface: &str) -> Result<()> {
        let entity = self.policy.find_entity(entity_id)
            .ok_or_else(|| EnforcementError::EntityNotFound(entity_id.to_string()))?;
        
        // Get rate limit for this interface
        let rate_limit = if let Some(limit_config) = entity.rate_limits.get(interface) {
            limit_config.operations_per_second
        } else {
            self.policy.settings.default_rate_limit
        };
        
        let mut counters = self.operation_counters.write().await;
        let key = format!("{}:{}", entity_id, interface);
        
        let now = Utc::now();
        let counter = counters.entry(key.clone()).or_insert(OperationCounter {
            count: 0,
            last_reset: now,
            rate_limit,
        });
        
        // Reset counter if a second has passed
        if (now - counter.last_reset).num_seconds() >= 1 {
            counter.count = 0;
            counter.last_reset = now;
        }
        
        // Check if limit exceeded
        if counter.count >= rate_limit {
            return Err(EnforcementError::RateLimitExceeded {
                entity: entity_id.to_string(),
                message: format!(
                    "{} operations/sec limit exceeded for {}",
                    rate_limit, interface
                ),
            });
        }
        
        counter.count += 1;
        Ok(())
    }
    
    /// Log an audit event
    pub async fn audit(
        &self,
        entity_id: &str,
        session_id: Uuid,
        interface: &str,
        operation: &str,
        success: bool,
        error: Option<String>,
    ) {
        let event = AuditEvent {
            timestamp: Utc::now(),
            entity_id: entity_id.to_string(),
            session_id,
            interface: interface.to_string(),
            operation: operation.to_string(),
            success,
            error,
        };
        
        let mut log = self.audit_log.write().await;
        log.push(event);
    }
    
    /// Get audit log for an entity
    pub async fn get_audit_log(&self, entity_id: Option<&str>, limit: usize) -> Vec<AuditEvent> {
        let log = self.audit_log.read().await;
        
        let filtered: Vec<_> = if let Some(id) = entity_id {
            log.iter()
                .filter(|e| e.entity_id == id)
                .cloned()
                .collect()
        } else {
            log.iter().cloned().collect()
        };
        
        // Return most recent entries
        filtered.into_iter()
            .rev()
            .take(limit)
            .collect()
    }
    
    /// List all entities in the policy
    pub fn list_entities(&self) -> Vec<String> {
        self.policy.entities.iter()
            .map(|e| e.id.clone())
            .collect()
    }
    
    /// Get entity configuration
    pub fn get_entity_config(&self, entity_id: &str) -> Option<&EntityConfig> {
        self.policy.find_entity(entity_id)
    }
    
    /// Get active sessions count
    pub async fn active_sessions_count(&self) -> usize {
        let sessions = self.sessions.read().await;
        let now = Utc::now();
        sessions.values()
            .filter(|s| s.expires_at > now)
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_enforcement_service() {
        let yaml = r#"
version: "1.0"
entities:
  - id: "test-entity"
    description: "Test"
    capabilities:
      crypto: true
      random: true
    rate_limits:
      crypto:
        operations_per_second: 100
        burst_size: 200
"#;
        
        let policy = PolicyConfig::from_yaml(yaml).unwrap();
        let service = EnforcementService::new(policy).unwrap();
        
        // Test session creation
        let session = service.create_session("test-entity").await.unwrap();
        assert_eq!(session.entity_id, "test-entity");
        assert_eq!(session.granted_capabilities.len(), 2);
        
        // Test capability check
        assert!(service.has_capability("test-entity", "crypto").unwrap());
        assert!(!service.has_capability("test-entity", "storage").unwrap());
        
        // Test HAL creation
        let hal = service.create_restricted_hal("test-entity").await.unwrap();
        assert!(hal.crypto.is_some());
        assert!(hal.random.is_some());
        assert!(hal.storage.is_none());
    }
}
