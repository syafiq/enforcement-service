//! Basic usage example for the HAL Enforcement Service

use hal_enforcement_service::{EnforcementService, PolicyConfig};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();
    
    println!("=== HAL Enforcement Service - Basic Usage Example ===\n");
    
    // 1. Create a simple policy
    let policy_yaml = r#"
version: "1.0"

settings:
  strict_mode: true

entities:
  - id: "crypto-worker"
    description: "Cryptographic operations service"
    capabilities:
      crypto: true
      random: true
      clock: true
    rate_limits:
      crypto:
        operations_per_second: 100
        burst_size: 200

  - id: "minimal-app"
    description: "Minimal access application"
    capabilities:
      random: true
      clock: true
    rate_limits:
      random:
        operations_per_second: 50
        burst_size: 100
"#;
    
    // 2. Load the policy
    println!("Loading policy configuration...");
    let policy = PolicyConfig::from_yaml(policy_yaml)?;
    let service = EnforcementService::new(policy)?;
    println!("✓ Policy loaded\n");
    
    // 3. List entities
    println!("Entities in policy:");
    for entity_id in service.list_entities() {
        if let Some(config) = service.get_entity_config(&entity_id) {
            println!("  - {} ({})", entity_id, config.description);
            println!("    Capabilities: {:?}", config.capabilities.list_granted());
        }
    }
    println!();
    
    // 4. Create session for crypto-worker
    println!("Creating session for crypto-worker...");
    let session = service.create_session("crypto-worker").await?;
    println!("✓ Session created: {}", session.session_id);
    println!("  Granted capabilities: {:?}", session.granted_capabilities);
    println!("  Expires at: {}\n", session.expires_at);
    
    // 5. The session carries the capability set; the HAL (WasmHAL's
    //    enforcement layer) uses it to allow or deny each interface.
    let granted = &session.granted_capabilities;
    for iface in ["crypto", "random", "clock", "storage"] {
        if granted.iter().any(|c| c == iface) {
            println!("✓ {} interface granted", iface);
        } else {
            println!("✓ {} interface correctly denied", iface);
        }
    }
    println!();

    // 6. A session for minimal-app gets fewer capabilities
    println!("Creating session for minimal-app...");
    let minimal = service.create_session("minimal-app").await?;
    println!("✓ Session created: {}", minimal.session_id);
    println!("  Granted capabilities: {:?}\n", minimal.granted_capabilities);

    // 7. Test rate limiting
    println!("Testing rate limiting...");
    for i in 0..5 {
        match service.check_rate_limit("crypto-worker", "crypto").await {
            Ok(()) => println!("  Operation {}: ✓ Allowed", i + 1),
            Err(e) => println!("  Operation {}: ✗ Denied - {}", i + 1, e),
        }
    }
    println!();
    
    // 8. Test capability checks
    println!("Testing capability checks...");
    println!("  crypto-worker has crypto? {}", 
        service.has_capability("crypto-worker", "crypto")?);
    println!("  crypto-worker has storage? {}", 
        service.has_capability("crypto-worker", "storage")?);
    println!("  minimal-app has random? {}", 
        service.has_capability("minimal-app", "random")?);
    println!("  minimal-app has crypto? {}", 
        service.has_capability("minimal-app", "crypto")?);
    println!();
    
    println!("=== Example Complete ===");
    
    Ok(())
}
