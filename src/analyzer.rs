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

// ---------------------------------------------------------------------------
// Optional: real OpenAI-backed analyser.
//
// Enabled with the `openai` cargo feature. Pulls in `reqwest` + `wasmparser`.
// The flow is:
//   1. Parse the WASM to extract its `(module, name)` imports.
//   2. Send the import list (as text) to OpenAI Chat Completions with a
//      strict JSON schema asking for one boolean per capability bucket.
//   3. Deserialise the response into `CapabilitiesConfig` and return it.
//
// The service does not interpret or upper-bound the verdict — the model is
// trusted, by design.
// ---------------------------------------------------------------------------
#[cfg(feature = "openai")]
pub use openai_impl::OpenAiAnalyzer;

#[cfg(feature = "openai")]
mod openai_impl {
    use super::*;
    use crate::error::EnforcementError;
    use serde::{Deserialize, Serialize};

    /// OpenAI Chat Completions-backed analyser.
    ///
    /// Reads the API key from the `OPENAI_API_KEY` environment variable by
    /// default, or accept one via [`OpenAiAnalyzer::with_api_key`]. The
    /// `model` argument from the policy YAML is forwarded as-is to OpenAI,
    /// so policies can pin a specific revision (e.g. `gpt-4o-2024-11-20`).
    pub struct OpenAiAnalyzer {
        client: reqwest::Client,
        api_key: String,
        endpoint: String,
    }

    impl OpenAiAnalyzer {
        /// Build from `$OPENAI_API_KEY`.
        pub fn from_env() -> Result<Self> {
            let api_key = std::env::var("OPENAI_API_KEY").map_err(|_| {
                EnforcementError::Policy("OPENAI_API_KEY not set".to_string())
            })?;
            Ok(Self::with_api_key(api_key))
        }

        pub fn with_api_key(api_key: String) -> Self {
            Self {
                client: reqwest::Client::new(),
                api_key,
                endpoint: "https://api.openai.com/v1/chat/completions".to_string(),
            }
        }

        /// Override the endpoint (useful for Azure / local proxies).
        pub fn with_endpoint(mut self, endpoint: String) -> Self {
            self.endpoint = endpoint;
            self
        }
    }

    /// Imports extracted from the WASM, fed into the prompt.
    fn extract_imports(wasm: &[u8]) -> Result<Vec<String>> {
        use wasmparser::{Parser, Payload};
        let mut imports = Vec::new();
        for payload in Parser::new(0).parse_all(wasm) {
            let payload = payload.map_err(|e| {
                EnforcementError::Policy(format!("invalid WASM: {e}"))
            })?;
            if let Payload::ImportSection(section) = payload {
                for imp in section {
                    let imp = imp.map_err(|e| {
                        EnforcementError::Policy(format!("bad import: {e}"))
                    })?;
                    imports.push(format!("{}::{}", imp.module, imp.name));
                }
            }
        }
        Ok(imports)
    }

    #[derive(Serialize)]
    struct ChatRequest<'a> {
        model: &'a str,
        messages: Vec<Message<'a>>,
        response_format: ResponseFormat,
        temperature: f32,
    }

    #[derive(Serialize)]
    struct Message<'a> {
        role: &'a str,
        content: String,
    }

    #[derive(Serialize)]
    struct ResponseFormat {
        #[serde(rename = "type")]
        kind: &'static str,
    }

    #[derive(Deserialize)]
    struct ChatResponse {
        choices: Vec<Choice>,
    }
    #[derive(Deserialize)]
    struct Choice {
        message: ChoiceMessage,
    }
    #[derive(Deserialize)]
    struct ChoiceMessage {
        content: String,
    }

    const SYSTEM_PROMPT: &str = "\
You are a capability analyser for a TEE workload. Given a list of WASM \
imports, decide which of the following capability buckets the workload \
actually needs:

  platform, capabilities, crypto, random, clock, storage, sockets, gpu, \
  resources, events, communication

Reply with a JSON object whose keys are exactly those names and values are \
booleans. Grant the minimum set sufficient for the imports shown. Do not \
include any commentary.";

    #[async_trait]
    impl LlmAnalyzer for OpenAiAnalyzer {
        async fn analyze(
            &self,
            model: &str,
            wasm_bytes: &[u8],
        ) -> Result<CapabilitiesConfig> {
            let imports = extract_imports(wasm_bytes)?;
            let user_content = if imports.is_empty() {
                "WASM module declares no imports.".to_string()
            } else {
                format!(
                    "WASM imports ({}):\n{}",
                    imports.len(),
                    imports.join("\n")
                )
            };

            let req = ChatRequest {
                model,
                messages: vec![
                    Message { role: "system", content: SYSTEM_PROMPT.to_string() },
                    Message { role: "user", content: user_content },
                ],
                response_format: ResponseFormat { kind: "json_object" },
                temperature: 0.0,
            };

            let resp = self
                .client
                .post(&self.endpoint)
                .bearer_auth(&self.api_key)
                .json(&req)
                .send()
                .await
                .map_err(|e| EnforcementError::Policy(format!("OpenAI request failed: {e}")))?;

            if !resp.status().is_success() {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                return Err(EnforcementError::Policy(format!(
                    "OpenAI returned {status}: {body}"
                )));
            }

            let parsed: ChatResponse = resp.json().await.map_err(|e| {
                EnforcementError::Policy(format!("OpenAI bad JSON: {e}"))
            })?;
            let content = parsed
                .choices
                .into_iter()
                .next()
                .ok_or_else(|| EnforcementError::Policy("OpenAI: no choices".to_string()))?
                .message
                .content;

            let caps: CapabilitiesConfig = serde_json::from_str(&content).map_err(|e| {
                EnforcementError::Policy(format!(
                    "model verdict not a CapabilitiesConfig: {e}; raw: {content}"
                ))
            })?;
            Ok(caps)
        }
    }
}
