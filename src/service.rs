//! Enforcement service implementation

use crate::analyzer::{LlmAnalyzer, MockAnalyzer};
use crate::config::{CapabilitiesConfig, EnforcementMode, EntityConfig, PolicyConfig};
use crate::error::{EnforcementError, Result};
use chrono::{DateTime, Duration, Utc};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Session information
#[derive(Debug, Clone)]
pub struct Session {
    pub session_id: Uuid,
    pub entity_id: String,
    pub granted_capabilities: Vec<String>,
    /// Which mode produced `granted_capabilities` for this session. Recorded
    /// on every session so audit logs can tell at a glance whether a grant
    /// came from a hand-written YAML or from an LLM analysis.
    pub policy_source: PolicySource,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub operation_count: u64,
}

/// Origin of a session's capability set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicySource {
    /// Capabilities came from the YAML `capabilities` block.
    Manual,
    /// Capabilities were produced by an LLM analyser. `model` is the
    /// identifier used; recorded for audit.
    Auto { model: String },
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
    /// Backend used for `EnforcementMode::Auto`. Defaults to [`MockAnalyzer`]
    /// so the service is usable out-of-the-box; deployments wire in a real
    /// LLM client via [`EnforcementService::with_analyzer`].
    analyzer: Arc<dyn LlmAnalyzer>,
}

#[derive(Debug)]
struct OperationCounter {
    count: u64,
    last_reset: DateTime<Utc>,
    #[allow(dead_code)]
    rate_limit: u64,
}

impl EnforcementService {
    /// Create a new enforcement service with the given policy. Uses the
    /// default [`MockAnalyzer`] for `Auto`-mode entities; replace it with
    /// [`Self::with_analyzer`] before serving real traffic.
    pub fn new(policy: PolicyConfig) -> Result<Self> {
        policy.validate()?;

        Ok(Self {
            policy,
            sessions: Arc::new(RwLock::new(HashMap::new())),
            audit_log: Arc::new(RwLock::new(Vec::new())),
            operation_counters: Arc::new(RwLock::new(HashMap::new())),
            analyzer: Arc::new(MockAnalyzer),
        })
    }

    /// Load policy from file (uses the default mock analyser).
    pub fn from_file(path: impl AsRef<std::path::Path>) -> Result<Self> {
        let policy = PolicyConfig::from_file(path)?;
        Self::new(policy)
    }

    /// Replace the LLM analyser used for `Auto`-mode entities.
    pub fn with_analyzer(mut self, analyzer: Arc<dyn LlmAnalyzer>) -> Self {
        self.analyzer = analyzer;
        self
    }

    /// Decide the capability set for an entity at session-creation time.
    ///
    /// * `Manual` mode: returns the YAML `capabilities` block verbatim.
    /// * `Auto`   mode: forwards the (decrypted) WASM bytes to the
    ///   configured [`LlmAnalyzer`] and returns its verdict. If
    ///   `wasm_bytes` is `None`, the call fails — auto mode requires a
    ///   workload to analyse.
    ///
    /// The two paths are mutually exclusive; there is no intersection or
    /// human-set upper bound applied to the auto verdict.
    pub async fn resolve_capabilities(
        &self,
        entity_id: &str,
        wasm_bytes: Option<&[u8]>,
    ) -> Result<(CapabilitiesConfig, PolicySource)> {
        let entity = self
            .policy
            .find_entity(entity_id)
            .ok_or_else(|| EnforcementError::EntityNotFound(entity_id.to_string()))?;

        match &entity.mode {
            EnforcementMode::Manual => {
                Ok((entity.capabilities.clone(), PolicySource::Manual))
            }
            EnforcementMode::Auto { model } => {
                let bytes = wasm_bytes.ok_or_else(|| {
                    EnforcementError::Policy(format!(
                        "entity '{}' is in auto mode but no WASM bytes were supplied",
                        entity_id
                    ))
                })?;
                let caps = self.analyzer.analyze(model, bytes).await?;
                Ok((caps, PolicySource::Auto { model: model.clone() }))
            }
        }
    }

    /// Create a new session for an entity. For `Manual` entities `wasm_bytes`
    /// may be `None`; for `Auto` entities it must be `Some`.
    pub async fn create_session_with_wasm(
        &self,
        entity_id: &str,
        wasm_bytes: Option<&[u8]>,
    ) -> Result<Session> {
        // Existence check (also gives us a nice error in non-strict mode).
        let _entity = self
            .policy
            .find_entity(entity_id)
            .ok_or_else(|| EnforcementError::EntityNotFound(entity_id.to_string()))?;

        let (caps, source) = self.resolve_capabilities(entity_id, wasm_bytes).await?;

        let session_id = Uuid::new_v4();
        let now = Utc::now();
        let expires_at = now + Duration::hours(24);

        let session = Session {
            session_id,
            entity_id: entity_id.to_string(),
            granted_capabilities: caps.list_granted(),
            policy_source: source.clone(),
            created_at: now,
            expires_at,
            operation_count: 0,
        };

        self.sessions.write().await.insert(session_id, session.clone());

        log::info!(
            "Created session {} for entity {} (source={:?}) with capabilities: {:?}",
            session_id,
            entity_id,
            source,
            session.granted_capabilities
        );

        Ok(session)
    }

    /// Convenience wrapper: create a session for a `Manual`-mode entity.
    /// Fails if the entity is in `Auto` mode (use
    /// [`Self::create_session_with_wasm`] for that).
    pub async fn create_session(&self, entity_id: &str) -> Result<Session> {
        self.create_session_with_wasm(entity_id, None).await
    }

    /// Get session information
    pub async fn get_session(&self, session_id: Uuid) -> Result<Session> {
        let sessions = self.sessions.read().await;
        let session = sessions
            .get(&session_id)
            .ok_or_else(|| EnforcementError::Session("Session not found".to_string()))?;

        if Utc::now() > session.expires_at {
            return Err(EnforcementError::Session("Session expired".to_string()));
        }

        Ok(session.clone())
    }

    /// Check if entity has a specific capability **in `Manual` mode**.
    ///
    /// In `Auto` mode the capability set is per-session (it depends on the
    /// WASM bytes), so this method has no answer outside of a session and
    /// returns an error.
    pub fn has_capability(&self, entity_id: &str, capability: &str) -> Result<bool> {
        let entity = self
            .policy
            .find_entity(entity_id)
            .ok_or_else(|| EnforcementError::EntityNotFound(entity_id.to_string()))?;

        match &entity.mode {
            EnforcementMode::Manual => Ok(entity.capabilities.has_capability(capability)),
            EnforcementMode::Auto { .. } => Err(EnforcementError::Policy(format!(
                "entity '{}' is in auto mode; capabilities are per-session",
                entity_id
            ))),
        }
    }

    /// Check rate limit for an entity
    pub async fn check_rate_limit(&self, entity_id: &str, interface: &str) -> Result<()> {
        let entity = self
            .policy
            .find_entity(entity_id)
            .ok_or_else(|| EnforcementError::EntityNotFound(entity_id.to_string()))?;

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

        if (now - counter.last_reset).num_seconds() >= 1 {
            counter.count = 0;
            counter.last_reset = now;
        }

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

        self.audit_log.write().await.push(event);
    }

    /// Get audit log for an entity
    pub async fn get_audit_log(&self, entity_id: Option<&str>, limit: usize) -> Vec<AuditEvent> {
        let log = self.audit_log.read().await;

        let filtered: Vec<_> = if let Some(id) = entity_id {
            log.iter().filter(|e| e.entity_id == id).cloned().collect()
        } else {
            log.iter().cloned().collect()
        };

        filtered.into_iter().rev().take(limit).collect()
    }

    /// List all entities in the policy
    pub fn list_entities(&self) -> Vec<String> {
        self.policy.entities.iter().map(|e| e.id.clone()).collect()
    }

    /// Get entity configuration
    pub fn get_entity_config(&self, entity_id: &str) -> Option<&EntityConfig> {
        self.policy.find_entity(entity_id)
    }

    /// Get active sessions count
    pub async fn active_sessions_count(&self) -> usize {
        let sessions = self.sessions.read().await;
        let now = Utc::now();
        sessions.values().filter(|s| s.expires_at > now).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn manual_mode_grants_yaml_capabilities() {
        let yaml = r#"
version: "1.0"
entities:
  - id: "test-entity"
    description: "Test"
    capabilities:
      crypto: true
      random: true
"#;

        let policy = PolicyConfig::from_yaml(yaml).unwrap();
        let service = EnforcementService::new(policy).unwrap();

        let session = service.create_session("test-entity").await.unwrap();
        assert_eq!(session.policy_source, PolicySource::Manual);
        assert_eq!(session.granted_capabilities.len(), 2);
        assert!(session.granted_capabilities.contains(&"crypto".to_string()));
        assert!(session.granted_capabilities.contains(&"random".to_string()));

        assert!(service.has_capability("test-entity", "crypto").unwrap());
        assert!(!service.has_capability("test-entity", "storage").unwrap());
    }

    #[tokio::test]
    async fn auto_mode_consults_analyzer_only() {
        // YAML grants nothing; auto mode should still produce a non-empty
        // capability set because the (mock) analyser grants all.
        let yaml = r#"
version: "1.0"
entities:
  - id: "auto-entity"
    description: "LLM-driven"
    mode:
      kind: auto
      model: "test-model"
    capabilities: {}
"#;

        let policy = PolicyConfig::from_yaml(yaml).unwrap();
        let service = EnforcementService::new(policy).unwrap();

        // No WASM bytes → auto mode must refuse.
        let err = service.create_session("auto-entity").await.unwrap_err();
        match err {
            EnforcementError::Policy(_) => {}
            e => panic!("expected Policy error, got {:?}", e),
        }

        // With WASM bytes → analyser produces caps; YAML is ignored.
        let wasm = b"\0asm\x01\x00\x00\x00";
        let session = service
            .create_session_with_wasm("auto-entity", Some(wasm))
            .await
            .unwrap();
        assert_eq!(
            session.policy_source,
            PolicySource::Auto {
                model: "test-model".to_string()
            }
        );
        // MockAnalyzer grants all 11 capabilities.
        assert_eq!(session.granted_capabilities.len(), 11);

        // has_capability is undefined in auto mode (per-session).
        assert!(service.has_capability("auto-entity", "crypto").is_err());
    }
}

