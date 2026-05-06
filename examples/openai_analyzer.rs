//! Real LLM-backed analyser, gated behind the `openai` feature.
//!
//! Works with any OpenAI-compatible chat-completions endpoint. Configure
//! via env vars:
//!     OPENAI_API_KEY  - required
//!     LLM_ENDPOINT    - optional, e.g. https://api.deepseek.com/v1/chat/completions
//!     LLM_MODEL       - optional, overrides the policy YAML model
//!
//! Examples:
//!     # OpenAI (default endpoint)
//!     export OPENAI_API_KEY=sk-...
//!     cargo run --example openai_analyzer --features openai -- module.wasm
//!
//!     # DeepSeek (OpenAI-compatible)
//!     export OPENAI_API_KEY=sk-deepseek-...
//!     export LLM_ENDPOINT=https://api.deepseek.com/v1/chat/completions
//!     export LLM_MODEL=deepseek-chat
//!     cargo run --example openai_analyzer --features openai -- module.wasm

#[cfg(not(feature = "openai"))]
fn main() {
    eprintln!("this example needs `--features openai`");
    std::process::exit(2);
}

#[cfg(feature = "openai")]
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    use hal_enforcement_service::analyzer::OpenAiAnalyzer;
    use hal_enforcement_service::{EnforcementService, PolicyConfig};
    use std::sync::Arc;

    let path = std::env::args().nth(1).ok_or_else(|| {
        anyhow::anyhow!("usage: openai_analyzer <module.wasm>")
    })?;
    let wasm = std::fs::read(&path)?;
    println!("loaded {} bytes from {}", wasm.len(), path);

    // Allow overriding model + endpoint from env (DeepSeek, Azure, local, ...).
    let model = std::env::var("LLM_MODEL").unwrap_or_else(|_| "gpt-4o-mini".to_string());
    let endpoint = std::env::var("LLM_ENDPOINT").ok();

    let yaml = format!(
        r#"
version: "1.0"
entities:
  - id: "auto-app"
    description: "LLM-driven"
    mode:
      kind: auto
      model: "{model}"
    capabilities: {{}}
"#
    );
    let policy = PolicyConfig::from_yaml(&yaml)?;

    let mut analyzer = OpenAiAnalyzer::from_env()?;
    if let Some(ep) = endpoint {
        println!("using endpoint: {}", ep);
        analyzer = analyzer.with_endpoint(ep);
    } else {
        println!("using default endpoint: https://api.openai.com/v1/chat/completions");
    }
    println!("using model:    {}", model);

    let service =
        EnforcementService::new(policy)?.with_analyzer(Arc::new(analyzer));

    let session = service
        .create_session_with_wasm("auto-app", Some(&wasm))
        .await?;

    println!("\npolicy_source: {:?}", session.policy_source);
    println!("granted ({}):", session.granted_capabilities.len());
    for c in &session.granted_capabilities {
        println!("  - {}", c);
    }
    Ok(())
}
