//! Run a workload against a policy. Prints PASS or DENIED.
//!
//! Demo 1 + Demo 2: load a policy, load a WASM, try to create a
//! session. The service extracts imports, maps each to a capability,
//! and refuses if any required capability isn't granted.
//!
//! Run:
//!     cargo run --example run_workload -- <policy.yaml> <entity-id> <module.wasm>

use hal_enforcement_service::{EnforcementService, PolicyConfig};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let policy_path = args
        .next()
        .ok_or_else(|| anyhow::anyhow!("usage: run_workload <policy> <entity> <wasm>"))?;
    let entity_id = args
        .next()
        .ok_or_else(|| anyhow::anyhow!("usage: run_workload <policy> <entity> <wasm>"))?;
    let wasm_path = args
        .next()
        .ok_or_else(|| anyhow::anyhow!("usage: run_workload <policy> <entity> <wasm>"))?;

    let policy = PolicyConfig::from_file(&policy_path)?;
    let wasm = std::fs::read(&wasm_path)?;

    println!("policy : {}", policy_path);
    println!("entity : {}", entity_id);
    println!("workload: {} ({} bytes)", wasm_path, wasm.len());

    // Show what the workload imports so the audience can see the inputs.
    let imports = hal_enforcement_service::extract_imports(&wasm)?;
    if imports.is_empty() {
        println!("imports : (none)");
    } else {
        println!("imports :");
        for (m, n) in &imports {
            let bucket = hal_enforcement_service::import_to_capability(m, n)
                .unwrap_or("(benign)");
            println!("  {}::{}  -> {}", m, n, bucket);
        }
    }

    let service = EnforcementService::new(policy)?;
    match service
        .create_session_with_wasm(&entity_id, Some(&wasm))
        .await
    {
        Ok(session) => {
            println!("\n\x1b[1;32m=> PASS\x1b[0m");
            println!("session_id    : {}", session.session_id);
            println!("policy_source : {:?}", session.policy_source);
            println!("granted       : {:?}", session.granted_capabilities);
        }
        Err(e) => {
            println!("\n\x1b[1;31m=> DENIED\x1b[0m");
            println!("{}", e);
            std::process::exit(1);
        }
    }
    Ok(())
}
