//! Freeze an LLM verdict into a manual-mode YAML policy.
//!
//! Demo 2 step 1: take a workload, ask the LLM what capabilities it
//! needs, write the verdict into a policy file. The resulting policy
//! is plain manual mode — no LLM at enforcement time.
//!
//! Run:
//!     export OPENAI_API_KEY=...
//!     export LLM_ENDPOINT=https://api.deepseek.com/v1/chat/completions
//!     export LLM_MODEL=deepseek-chat
//!     cargo run --example freeze_policy --features openai -- \
//!         demo-wasms/sockets-app.wasm policies/frozen.yaml my-app

#[cfg(not(feature = "openai"))]
fn main() {
    eprintln!("this example needs `--features openai`");
    std::process::exit(2);
}

#[cfg(feature = "openai")]
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    use hal_enforcement_service::analyzer::{LlmAnalyzer, OpenAiAnalyzer};

    let mut args = std::env::args().skip(1);
    let wasm_path = args
        .next()
        .ok_or_else(|| anyhow::anyhow!("usage: freeze_policy <wasm> <out.yaml> <entity-id>"))?;
    let out_path = args
        .next()
        .ok_or_else(|| anyhow::anyhow!("usage: freeze_policy <wasm> <out.yaml> <entity-id>"))?;
    let entity_id = args.next().unwrap_or_else(|| "frozen-app".to_string());

    let wasm = std::fs::read(&wasm_path)?;
    let model = std::env::var("LLM_MODEL").unwrap_or_else(|_| "gpt-4o-mini".to_string());

    let mut analyzer = OpenAiAnalyzer::from_env()?;
    if let Ok(ep) = std::env::var("LLM_ENDPOINT") {
        analyzer = analyzer.with_endpoint(ep);
    }

    println!("analysing {} with {}...", wasm_path, model);
    let caps = analyzer.analyze(&model, &wasm).await?;
    let granted = caps.list_granted();
    println!("model granted: {:?}", granted);

    // Render a tiny YAML policy: one entity, manual mode, frozen caps.
    let mut yaml = String::new();
    yaml.push_str("version: \"1.0\"\n");
    yaml.push_str("# Frozen from LLM verdict — see audit trail below.\n");
    yaml.push_str(&format!(
        "# source_wasm: {}\n# model: {}\n# generated: {}\n",
        wasm_path,
        model,
        chrono::Utc::now().to_rfc3339()
    ));
    yaml.push_str("entities:\n");
    yaml.push_str(&format!("  - id: \"{}\"\n", entity_id));
    yaml.push_str(&format!(
        "    description: \"Frozen from {}\"\n",
        wasm_path
    ));
    yaml.push_str("    capabilities:\n");
    for c in &granted {
        yaml.push_str(&format!("      {}: true\n", c));
    }
    if granted.is_empty() {
        yaml.push_str("      {}\n");
    }

    std::fs::write(&out_path, &yaml)?;
    println!("wrote {} ({} bytes)", out_path, yaml.len());
    println!("\n--- {} ---", out_path);
    print!("{}", yaml);
    Ok(())
}
