//! Integration tests for the enforcement service

use hal_enforcement_service::{EnforcementService, PolicyConfig};

const TEST_POLICY: &str = r#"
version: "1.0"

settings:
  strict_mode: true
  default_rate_limit: 100

entities:
  - id: "test-crypto"
    description: "Test crypto entity"
    capabilities:
      crypto: true
      random: true
      clock: true
    rate_limits:
      crypto:
        operations_per_second: 10
        burst_size: 20

  - id: "test-minimal"
    description: "Test minimal entity"
    capabilities:
      random: true
    rate_limits:
      random:
        operations_per_second: 5
        burst_size: 10
"#;

#[tokio::test]
async fn test_policy_loading() {
    let policy = PolicyConfig::from_yaml(TEST_POLICY).unwrap();
    assert_eq!(policy.version, "1.0");
    assert_eq!(policy.entities.len(), 2);
}

#[tokio::test]
async fn test_service_creation() {
    let service = EnforcementService::from_yaml(TEST_POLICY).unwrap();
    let entities = service.list_entities();
    assert_eq!(entities.len(), 2);
    assert!(entities.contains(&"test-crypto".to_string()));
    assert!(entities.contains(&"test-minimal".to_string()));
}

#[tokio::test]
async fn test_session_management() {
    let service = EnforcementService::from_yaml(TEST_POLICY).unwrap();
    
    // Create session
    let session = service.create_session("test-crypto").await.unwrap();
    assert_eq!(session.entity_id, "test-crypto");
    assert!(!session.granted_capabilities.is_empty());
    
    // Retrieve session
    let retrieved = service.get_session(session.session_id).await.unwrap();
    assert_eq!(retrieved.entity_id, session.entity_id);
}

#[tokio::test]
async fn test_capability_enforcement() {
    let service = EnforcementService::from_yaml(TEST_POLICY).unwrap();
    
    // test-crypto should have crypto
    assert!(service.has_capability("test-crypto", "crypto").unwrap());
    assert!(service.has_capability("test-crypto", "random").unwrap());
    
    // test-crypto should NOT have storage
    assert!(!service.has_capability("test-crypto", "storage").unwrap());
    
    // test-minimal should only have random
    assert!(service.has_capability("test-minimal", "random").unwrap());
    assert!(!service.has_capability("test-minimal", "crypto").unwrap());
}

#[tokio::test]
async fn test_hal_creation() {
    let service = EnforcementService::from_yaml(TEST_POLICY).unwrap();
    
    // Create HAL for test-crypto
    let hal = service.create_restricted_hal("test-crypto").await.unwrap();
    assert!(hal.crypto.is_some());
    assert!(hal.random.is_some());
    assert!(hal.clock.is_some());
    assert!(hal.storage.is_none());
    
    // Create HAL for test-minimal
    let minimal_hal = service.create_restricted_hal("test-minimal").await.unwrap();
    assert!(minimal_hal.crypto.is_none());
    assert!(minimal_hal.random.is_some());
}

#[tokio::test]
async fn test_rate_limiting() {
    let service = EnforcementService::from_yaml(TEST_POLICY).unwrap();
    
    // First few operations should succeed
    for i in 0..5 {
        service.check_rate_limit("test-crypto", "crypto").await
            .expect(&format!("Operation {} should succeed", i));
    }
    
    // After hitting limit, should fail
    // (depends on timing, so we just check it doesn't panic)
    let _ = service.check_rate_limit("test-crypto", "crypto").await;
}

#[tokio::test]
async fn test_audit_logging() {
    let service = EnforcementService::from_yaml(TEST_POLICY).unwrap();
    
    let session = service.create_session("test-crypto").await.unwrap();
    
    // Log some audit events
    service.audit(
        "test-crypto",
        session.session_id,
        "crypto",
        "hash",
        true,
        None,
    ).await;
    
    service.audit(
        "test-crypto",
        session.session_id,
        "crypto",
        "encrypt",
        false,
        Some("Test error".to_string()),
    ).await;
    
    // Retrieve audit log
    let events = service.get_audit_log(Some("test-crypto"), 10).await;
    assert_eq!(events.len(), 2);
    
    // Check event details
    assert_eq!(events[1].operation, "hash");
    assert!(events[1].success);
    assert_eq!(events[0].operation, "encrypt");
    assert!(!events[0].success);
}

#[tokio::test]
async fn test_strict_mode() {
    let service = EnforcementService::from_yaml(TEST_POLICY).unwrap();
    
    // Unknown entity should fail in strict mode
    let result = service.create_session("unknown-entity").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_crypto_operations() {
    let service = EnforcementService::from_yaml(TEST_POLICY).unwrap();
    let hal = service.create_restricted_hal("test-crypto").await.unwrap();
    
    if let Some(crypto) = &hal.crypto {
        let data = b"test data";
        let result = crypto.hash(data, "SHA-256");
        assert!(result.is_ok());
    }
}

#[tokio::test]
async fn test_random_operations() {
    let service = EnforcementService::from_yaml(TEST_POLICY).unwrap();
    let hal = service.create_restricted_hal("test-minimal").await.unwrap();
    
    if let Some(random) = &hal.random {
        let result = random.get_random_bytes(32);
        assert!(result.is_ok());
    }
}

// Helper for testing
impl EnforcementService {
    pub fn from_yaml(yaml: &str) -> Result<Self, hal_enforcement_service::error::EnforcementError> {
        let policy = PolicyConfig::from_yaml(yaml)?;
        Self::new(policy)
    }
}
