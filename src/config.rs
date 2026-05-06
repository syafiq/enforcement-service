//! Configuration loading and validation

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use crate::error::{EnforcementError, Result};

/// Complete policy configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyConfig {
    pub version: String,
    
    #[serde(default)]
    pub settings: GlobalSettings,
    
    pub entities: Vec<EntityConfig>,
}

/// Global settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalSettings {
    #[serde(default = "default_rate_limit")]
    pub default_rate_limit: u64,
    
    #[serde(default = "default_audit_log")]
    pub audit_log_path: String,
    
    #[serde(default)]
    pub strict_mode: bool,
}

fn default_rate_limit() -> u64 { 100 }
fn default_audit_log() -> String { "./logs/audit.log".to_string() }

impl Default for GlobalSettings {
    fn default() -> Self {
        Self {
            default_rate_limit: default_rate_limit(),
            audit_log_path: default_audit_log(),
            strict_mode: false,
        }
    }
}

/// Entity configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityConfig {
    pub id: String,
    
    #[serde(default)]
    pub description: String,

    /// How this entity's capabilities are decided.
    ///
    /// * `Manual` (default) — capabilities come from the YAML `capabilities`
    ///   block below. The operator is responsible for getting it right; if
    ///   the WASM workload tries to use something that wasn't granted it
    ///   simply fails at the HAL boundary.
    /// * `Auto`  — capabilities are derived at session-creation time by an
    ///   LLM analysing the (decrypted) WASM bytes. The YAML `capabilities`
    ///   block is **not** consulted in this mode; the model is the sole
    ///   policy source. This implements the "no upper bound, model is
    ///   trusted" position agreed for ELASTIC.
    ///
    /// The two modes are mutually exclusive; there is no intersection.
    #[serde(default)]
    pub mode: EnforcementMode,

    /// Capabilities granted to this entity in `Manual` mode. Ignored when
    /// `mode = Auto`.
    #[serde(default)]
    pub capabilities: CapabilitiesConfig,
    
    #[serde(default)]
    pub rate_limits: HashMap<String, RateLimitConfig>,
    
    #[serde(default)]
    pub quotas: HashMap<String, QuotaConfig>,
    
    #[serde(default)]
    pub can_grant: bool,
}

/// How an entity's capability set is decided.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum EnforcementMode {
    /// Capabilities come from the YAML `capabilities` field.
    Manual,
    /// Capabilities are produced by an LLM analyser, given the WASM bytes
    /// at session-creation time. `model` is an opaque identifier (e.g.
    /// `"gpt-4o-2024-11-20"`, `"claude-3-7-sonnet"`); interpretation is up
    /// to the configured `LlmAnalyzer` implementation.
    Auto {
        #[serde(default = "default_model")]
        model: String,
    },
}

fn default_model() -> String {
    "default".to_string()
}

impl Default for EnforcementMode {
    fn default() -> Self {
        EnforcementMode::Manual
    }
}

/// Capabilities configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CapabilitiesConfig {
    #[serde(default)]
    pub platform: bool,
    
    #[serde(default)]
    pub capabilities: bool,
    
    #[serde(default)]
    pub crypto: bool,
    
    #[serde(default)]
    pub random: bool,
    
    #[serde(default)]
    pub clock: bool,
    
    #[serde(default)]
    pub storage: bool,
    
    #[serde(default)]
    pub sockets: bool,
    
    #[serde(default)]
    pub gpu: bool,
    
    #[serde(default)]
    pub resources: bool,
    
    #[serde(default)]
    pub events: bool,
    
    #[serde(default)]
    pub communication: bool,
}

impl CapabilitiesConfig {
    pub fn all() -> Self {
        Self {
            platform: true,
            capabilities: true,
            crypto: true,
            random: true,
            clock: true,
            storage: true,
            sockets: true,
            gpu: true,
            resources: true,
            events: true,
            communication: true,
        }
    }
    
    pub fn none() -> Self {
        Self::default()
    }
    
    pub fn has_capability(&self, cap: &str) -> bool {
        match cap {
            "platform" => self.platform,
            "capabilities" => self.capabilities,
            "crypto" => self.crypto,
            "random" => self.random,
            "clock" => self.clock,
            "storage" => self.storage,
            "sockets" => self.sockets,
            "gpu" => self.gpu,
            "resources" => self.resources,
            "events" => self.events,
            "communication" => self.communication,
            _ => false,
        }
    }
    
    pub fn list_granted(&self) -> Vec<String> {
        let mut granted = Vec::new();
        if self.platform { granted.push("platform".to_string()); }
        if self.capabilities { granted.push("capabilities".to_string()); }
        if self.crypto { granted.push("crypto".to_string()); }
        if self.random { granted.push("random".to_string()); }
        if self.clock { granted.push("clock".to_string()); }
        if self.storage { granted.push("storage".to_string()); }
        if self.sockets { granted.push("sockets".to_string()); }
        if self.gpu { granted.push("gpu".to_string()); }
        if self.resources { granted.push("resources".to_string()); }
        if self.events { granted.push("events".to_string()); }
        if self.communication { granted.push("communication".to_string()); }
        granted
    }
}

/// Rate limit configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    pub operations_per_second: u64,
    
    #[serde(default)]
    pub burst_size: u64,
}

impl RateLimitConfig {
    pub fn new(ops_per_sec: u64) -> Self {
        Self {
            operations_per_second: ops_per_sec,
            burst_size: ops_per_sec * 2, // Default burst to 2x
        }
    }
}

/// Quota configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuotaConfig {
    #[serde(default)]
    pub max_bytes: Option<u64>,
    
    #[serde(default)]
    pub max_operations: Option<u64>,
}

impl PolicyConfig {
    /// Load policy from YAML file
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self> {
        let content = std::fs::read_to_string(path.as_ref())?;
        let config: PolicyConfig = serde_yaml::from_str(&content)?;
        config.validate()?;
        Ok(config)
    }
    
    /// Load policy from YAML string
    pub fn from_yaml(yaml: &str) -> Result<Self> {
        let config: PolicyConfig = serde_yaml::from_str(yaml)?;
        config.validate()?;
        Ok(config)
    }
    
    /// Validate the policy configuration
    pub fn validate(&self) -> Result<()> {
        // Check version
        if self.version.is_empty() {
            return Err(EnforcementError::Config("Version is required".to_string()));
        }
        
        // Check for duplicate entity IDs
        let mut ids = std::collections::HashSet::new();
        for entity in &self.entities {
            if !ids.insert(&entity.id) {
                return Err(EnforcementError::Config(
                    format!("Duplicate entity ID: {}", entity.id)
                ));
            }
        }
        
        // Validate each entity
        for entity in &self.entities {
            entity.validate()?;
        }
        
        Ok(())
    }
    
    /// Find entity by ID
    pub fn find_entity(&self, id: &str) -> Option<&EntityConfig> {
        self.entities.iter().find(|e| e.id == id)
    }
}

impl EntityConfig {
    /// Validate entity configuration
    pub fn validate(&self) -> Result<()> {
        if self.id.is_empty() {
            return Err(EnforcementError::Config("Entity ID cannot be empty".to_string()));
        }
        
        // Validate rate limits
        for (cap, limit) in &self.rate_limits {
            if limit.operations_per_second == 0 {
                return Err(EnforcementError::Config(
                    format!("Rate limit for {} cannot be zero", cap)
                ));
            }
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_parse_policy() {
        let yaml = r#"
version: "1.0"
entities:
  - id: "test-entity"
    description: "Test entity"
    capabilities:
      crypto: true
      random: true
    rate_limits:
      crypto:
        operations_per_second: 100
        burst_size: 200
"#;
        
        let config = PolicyConfig::from_yaml(yaml).unwrap();
        assert_eq!(config.version, "1.0");
        assert_eq!(config.entities.len(), 1);
        assert_eq!(config.entities[0].id, "test-entity");
        assert!(config.entities[0].capabilities.crypto);
        assert!(config.entities[0].capabilities.random);
        assert!(!config.entities[0].capabilities.storage);
    }
    
    #[test]
    fn test_capability_list() {
        let caps = CapabilitiesConfig {
            crypto: true,
            random: true,
            clock: true,
            ..Default::default()
        };
        
        let granted = caps.list_granted();
        assert_eq!(granted.len(), 3);
        assert!(granted.contains(&"crypto".to_string()));
        assert!(granted.contains(&"random".to_string()));
        assert!(granted.contains(&"clock".to_string()));
    }
}
