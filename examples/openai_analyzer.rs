//! Real OpenAI-backed analyser, gated behind the `openai` feature.
//!
//! Run with:
//!     export OPENAI_API_KEY=sk-...
//!     cargo run --example openai_analyzer --features openai -- path/to/module.wasm
//!
//! The example:
//!   1. Loads a tiny YAML policy with a single auto-mode entity.
//!   2. Wires `OpenAiAnalyzer::from_env()` into the service.
//!   3. Reads the WASM file given on the command line.
//!   4. Creates a session and prints what the model granted.

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

    let yaml = r#"
version: "1.0"
entities:
  - id: "auto-app"
    description: "LLM-driven"
    mode:
      kind: auto
      model: "gpt-4o-mini"
    capabilities: {}
"#;
    let policy = PolicyConfig::from_yaml(yaml)?;
    let service = EnforcementService::new(policy)?
        .with_analyzer(Arc::new(OpenAiAnalyzer::from_env()?));

    let session = service
        .create_session_with_wasm("auto-app", Some(&wasm))
        .await?;

    println!("policy_source: {:?}", session.policy_source);
    println!("granted ({}):", session.granted_capabilities.len());
    for c in &session.granted_capabilities {
        println!("  - {}", c);
    }
    Ok(())
}
