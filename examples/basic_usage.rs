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
    
    // 5. Create restricted HAL for crypto-worker
    println!("Creating restricted HAL for crypto-worker...");
    let crypto_hal = service.create_restricted_hal("crypto-worker").await?;
    
    if crypto_hal.crypto.is_some() {
        println!("✓ Crypto interface granted");
    }
    if crypto_hal.random.is_some() {
        println!("✓ Random interface granted");
    }
    if crypto_hal.clock.is_some() {
        println!("✓ Clock interface granted");
    }
    if crypto_hal.storage.is_none() {
        println!("✓ Storage interface correctly denied");
    }
    println!();
    
    // 6. Test crypto operations
    if let Some(crypto) = &crypto_hal.crypto {
        println!("Testing crypto operations...");
        let data = b"Hello, HAL Enforcement!";
        
        match crypto.hash(data, "SHA-256") {
            Ok(hash) => {
                println!("✓ SHA-256 hash computed: {} bytes", hash.len());
            }
            Err(e) => {
                println!("✗ Hash operation failed: {}", e);
            }
        }
    }
    
    // 7. Test random operations
    if let Some(random) = &crypto_hal.random {
        println!("Testing random operations...");
        
        match random.get_random_bytes(32) {
            Ok(bytes) => {
                println!("✓ Generated {} random bytes", bytes.len());
            }
            Err(e) => {
                println!("✗ Random operation failed: {}", e);
            }
        }
    }
    
    // 8. Test clock operations
    if let Some(clock) = &crypto_hal.clock {
        println!("Testing clock operations...");
        
        match clock.system_time() {
            Ok((secs, nanos)) => {
                println!("✓ System time: {}.{:09}s", secs, nanos);
            }
            Err(e) => {
                println!("✗ Clock operation failed: {}", e);
            }
        }
    }
    println!();
    
    // 9. Create HAL for minimal-app (should have fewer capabilities)
    println!("Creating restricted HAL for minimal-app...");
    let minimal_hal = service.create_restricted_hal("minimal-app").await?;
    
    if minimal_hal.crypto.is_none() {
        println!("✓ Crypto interface correctly denied for minimal-app");
    }
    if minimal_hal.random.is_some() {
        println!("✓ Random interface granted for minimal-app");
    }
    println!();
    
    // 10. Test rate limiting
    println!("Testing rate limiting...");
    for i in 0..5 {
        match service.check_rate_limit("crypto-worker", "crypto").await {
            Ok(()) => println!("  Operation {}: ✓ Allowed", i + 1),
            Err(e) => println!("  Operation {}: ✗ Denied - {}", i + 1, e),
        }
    }
    println!();
    
    // 11. Test capability checks
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
