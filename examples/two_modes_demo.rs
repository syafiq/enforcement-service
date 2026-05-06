//! Two-modes demo for the HAL Enforcement Service.
//!
//! Pacing: between sections the example pauses for ENTER so you can
//! narrate without racing the terminal. Set `DEMO_AUTO=1` to skip pauses
//! (useful for dry-runs / CI).

use hal_enforcement_service::{
    EnforcementError, EnforcementService, PolicyConfig, PolicySource,
};
use std::io::{self, BufRead, Write};

fn pause(label: &str) {
    if std::env::var("DEMO_AUTO").is_ok() {
        println!();
        return;
    }
    print!("\n\x1b[2m[press ENTER for {}]\x1b[0m ", label);
    io::stdout().flush().ok();
    let mut buf = String::new();
    io::stdin().lock().read_line(&mut buf).ok();
}

fn header(n: u8, title: &str) {
    println!("\n\x1b[1;36m=== {}. {} ===\x1b[0m", n, title);
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("\x1b[1mHAL Enforcement Service — two-modes demo\x1b[0m");

    // ------------------------------------------------------------------
    header(1, "The policy file");
    // ------------------------------------------------------------------
    let policy_yaml = r#"
version: "1.0"
settings:
  strict_mode: true
entities:
  # Manual mode (default): capabilities come from this file, verbatim.
  - id: "crypto-worker"
    description: "Hand-specified cryptographic worker"
    capabilities:
      crypto: true
      random: true
      clock: true

  # Auto mode: an LLM analyses the WASM and decides. YAML caps ignored.
  - id: "auto-analysed-app"
    description: "LLM-driven capability resolution"
    mode:
      kind: auto
      model: "gpt-4o-2024-11-20"
    capabilities: {}
"#;
    println!("{}", policy_yaml.trim());
    pause("manual mode");

    let policy = PolicyConfig::from_yaml(policy_yaml)?;
    let service = EnforcementService::new(policy)?;

    // ------------------------------------------------------------------
    header(2, "Manual mode — deterministic, auditable");
    // ------------------------------------------------------------------
    let session = service.create_session("crypto-worker").await?;
    println!("entity            : {}", session.entity_id);
    println!("policy_source     : {:?}", session.policy_source);
    println!("granted capabilities:");
    for c in &session.granted_capabilities {
        println!("  - {}", c);
    }
    assert_eq!(session.policy_source, PolicySource::Manual);
    pause("auto mode without a workload");

    // ------------------------------------------------------------------
    header(3, "Auto mode — refuses without a workload");
    // ------------------------------------------------------------------
    println!("calling create_session(\"auto-analysed-app\")  // no WASM bytes");
    match service.create_session("auto-analysed-app").await {
        Ok(_) => panic!("auto mode must refuse without bytes"),
        Err(EnforcementError::Policy(msg)) => {
            println!("\x1b[33m=> refused:\x1b[0m {}", msg);
        }
        Err(e) => panic!("unexpected error: {e:?}"),
    }
    pause("auto mode with a workload");

    // ------------------------------------------------------------------
    header(4, "Auto mode — analyser produces capabilities");
    // ------------------------------------------------------------------
    // Minimal valid WASM module header. In a real demo, hand it a full
    // module and a real analyser via `EnforcementService::with_analyzer`.
    let wasm_bytes: &[u8] = b"\0asm\x01\x00\x00\x00";
    println!("calling create_session_with_wasm(\"auto-analysed-app\", {} bytes)",
        wasm_bytes.len());

    let session = service
        .create_session_with_wasm("auto-analysed-app", Some(wasm_bytes))
        .await?;
    println!("entity            : {}", session.entity_id);
    println!("policy_source     : {:?}", session.policy_source);
    println!("granted capabilities ({}):", session.granted_capabilities.len());
    for c in &session.granted_capabilities {
        println!("  - {}", c);
    }
    match &session.policy_source {
        PolicySource::Auto { model } => {
            println!("\nThe YAML capabilities block was empty. Every grant above");
            println!("was decided by model `{}` from the WASM bytes alone.", model);
        }
        _ => panic!("expected Auto"),
    }
    pause("audit & wrap-up");

    // ------------------------------------------------------------------
    header(5, "Audit — every session traces back to its source");
    // ------------------------------------------------------------------
    println!("Manual session  -> source = Manual          (points to the YAML)");
    println!("Auto   session  -> source = Auto {{ model }}   (points to model + workload)");
    println!();
    println!("\x1b[1mTwo modes. No intersection. Pick the one that fits the workload.\x1b[0m");
    Ok(())
}
