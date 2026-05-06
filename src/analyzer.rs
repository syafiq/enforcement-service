//! WASM analysis backend used by `EnforcementMode::Auto`.
//!
//! The contract is intentionally tiny:
//!
//! * Input  — a model identifier (so the operator can pick which LLM /
//!   analyser to delegate to) and the **decrypted** WASM bytes.
//! * Output — a [`CapabilitiesConfig`], i.e. the exact same structure
//!   `Manual` mode produces from YAML.
//!
//! The enforcement service does not interpret, validate or upper-bound the
//! result. Per the ELASTIC design discussion, the model is trusted; any
//! mistakes it makes are a model-quality problem, not a service problem.
//!
//! This module ships a [`MockAnalyzer`] which always returns
//! `CapabilitiesConfig::all()` — useful for tests and as a placeholder
//! until a real analyser is wired in.

use crate::config::CapabilitiesConfig;
use crate::error::Result;
use async_trait::async_trait;

/// Backend that derives a capability set from WASM bytes.
#[async_trait]
pub trait LlmAnalyzer: Send + Sync {
    /// Analyse `wasm_bytes` and return the capability set the workload
    /// should be granted. `model` is the opaque identifier that came from
    /// the entity's YAML `mode: { kind: auto, model: "..." }`.
    async fn analyze(&self, model: &str, wasm_bytes: &[u8]) -> Result<CapabilitiesConfig>;
}

/// Test/placeholder analyser. Grants every capability, regardless of input.
///
/// Useful for:
///   * Unit tests of the dispatch logic.
///   * Local development before a real LLM client is integrated.
pub struct MockAnalyzer;

#[async_trait]
impl LlmAnalyzer for MockAnalyzer {
    async fn analyze(&self, _model: &str, _wasm_bytes: &[u8]) -> Result<CapabilitiesConfig> {
        Ok(CapabilitiesConfig::all())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn mock_analyzer_grants_all() {
        let a = MockAnalyzer;
        let caps = a.analyze("any-model", b"\0asm\x01\x00\x00\x00").await.unwrap();
        // Every flag should be set.
        assert!(caps.crypto);
        assert!(caps.storage);
        assert!(caps.sockets);
        assert!(caps.platform);
    }
}
