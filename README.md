# HAL Enforcement Service

**Policy-based access control service for ELASTIC TEE HAL**

The HAL Enforcement Service is a standalone gateway that provides capability-based access control to WASMHAL components. It loads YAML policy configurations from a centralized authority and grants restricted HAL access to different entities based on their permissions.

## Features

- **YAML Policy Configuration** - Define entity capabilities in human-readable YAML
- **Capability-Based Security** - Fine-grained control over HAL interface access
- **Rate Limiting** - Per-entity operation limits
- **Quota Management** - Data and operation quotas
- **Audit Logging** - Complete audit trail of all operations
- **HTTP API** - REST API for entity HAL access
- **Multi-Entity Support** - Manage multiple entities with different policies

## Quick Start

### 1. Define Policies

Create a policy file `policies/my-policies.yaml`:

```yaml
version: "1.0"
entities:
  - id: "crypto-worker"
    description: "Cryptographic operations service"
    capabilities:
      crypto: true
      random: true
      clock: true
    rate_limits:
      crypto:
        operations_per_second: 1000
        burst_size: 2000
    quotas:
      crypto:
        max_bytes: 10485760  # 10 MB
        max_operations: 100000

  - id: "attestation-service"
    description: "Platform attestation only"
    capabilities:
      platform: true
      capabilities: true
    rate_limits:
      platform:
        operations_per_second: 10
        burst_size: 20

  - id: "untrusted-app"
    description: "Limited access for untrusted workloads"
    capabilities:
      random: true
    rate_limits:
      random:
        operations_per_second: 100
        burst_size: 150
```

### 2. Run the Service

```bash
# Start the enforcement service
cd enforcement-service
cargo run -- --policy policies/my-policies.yaml --port 8080

# Or install and run
cargo install --path .
hal-enforcer --policy policies/my-policies.yaml
```

### 3. Use from WASM Components

```rust
// In your WASM component
use hal_enforcement_client::EnforcementClient;

let client = EnforcementClient::new("http://localhost:8080", "crypto-worker");

// Request HAL access (automatically enforced based on policy)
let hal = client.get_hal_access().await?;

// Use allowed interfaces
if let Some(crypto) = hal.crypto {
    let hash = crypto.hash(data, "SHA-256")?;
}

// Attempting to use denied interfaces returns None
assert!(hal.storage.is_none()); // Not granted in policy
```

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Centralized Authority                   │
│                  (Policy Configuration)                    │
└─────────────────────────────────────────────────────────────┘
                          │
                          │ YAML Policy Files
                          ▼
┌─────────────────────────────────────────────────────────────┐
│              HAL Enforcement Service (This)                │
│                                                             │
│  ├─ Policy Loader       ─ Load and validate YAML          │
│  ├─ Entity Manager      ─ Track entity sessions           │
│  ├─ Access Controller   ─ Create restricted HAL instances │
│  ├─ Rate Limiter        ─ Enforce operation limits        │
│  ├─ Quota Tracker       ─ Monitor usage quotas            │
│  └─ Audit Logger        ─ Log all operations              │
└─────────────────────────────────────────────────────────────┘
                          │
                          │ HTTP API (REST)
                          ▼
┌─────────────────────────────────────────────────────────────┐
│                    WASM Components                         │
│                                                             │
│  Entity A: crypto-worker    (crypto + random + clock)      │
│  Entity B: attestation-svc  (platform + capabilities)      │
│  Entity C: untrusted-app    (random only)                  │
└─────────────────────────────────────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────────┐
│                  ELASTIC TEE HAL                           │
│          (Platform, Crypto, Storage, etc.)                 │
└─────────────────────────────────────────────────────────────┘
```

## Policy Format

### Complete Policy Example

```yaml
version: "1.0"

# Global settings (optional)
settings:
  default_rate_limit: 100
  audit_log_path: "./logs/audit.log"
  strict_mode: true  # Deny access if entity not found

# Entity definitions
entities:
  # Full-privilege supervisor
  - id: "supervisor"
    description: "Administrative entity with full access"
    capabilities:
      platform: true
      capabilities: true
      crypto: true
      random: true
      clock: true
      storage: true
      sockets: true
      gpu: true
      resources: true
      events: true
      communication: true
    can_grant: true  # Can modify other entities' permissions

  # Specialized services
  - id: "crypto-worker"
    description: "Dedicated cryptographic service"
    capabilities:
      crypto: true
      random: true
      clock: true
    rate_limits:
      crypto:
        operations_per_second: 1000
        burst_size: 2000
    quotas:
      crypto:
        max_bytes: 10485760      # 10 MB
        max_operations: 100000

  - id: "storage-service"
    description: "Encrypted storage service"
    capabilities:
      storage: true
      crypto: true
      clock: true
    rate_limits:
      storage:
        operations_per_second: 500
        burst_size: 1000
    quotas:
      storage:
        max_bytes: 1073741824    # 1 GB
        max_operations: 50000

  - id: "network-proxy"
    description: "Network communication proxy"
    capabilities:
      sockets: true
      crypto: true  # For TLS
      clock: true
    rate_limits:
      sockets:
        operations_per_second: 10000
        burst_size: 20000
```

## API Endpoints

### Request HAL Access

```
POST /api/v1/hal/access
Content-Type: application/json

{
  "entity_id": "crypto-worker",
  "session_id": "optional-session-id"
}

Response:
{
  "session_id": "uuid-v4",
  "granted_capabilities": ["crypto", "random", "clock"],
  "rate_limits": { ... },
  "expires_at": "2026-02-16T12:00:00Z"
}
```

### Perform HAL Operation

```
POST /api/v1/hal/execute
Content-Type: application/json

{
  "session_id": "uuid-v4",
  "interface": "crypto",
  "operation": "hash",
  "parameters": {
    "data": "base64-encoded-data",
    "algorithm": "SHA-256"
  }
}
```

### Get Audit Log

```
GET /api/v1/audit?entity_id=crypto-worker&limit=100
```

## CLI Usage

```bash
# Start service with policy file
hal-enforcer --policy policies/production.yaml --port 8080

# Enable verbose logging
hal-enforcer --policy policies/dev.yaml --verbose

# Run with custom audit log location
hal-enforcer --policy policies/prod.yaml --audit-log /var/log/hal-audit.log

# Validate policy without starting service
hal-enforcer --policy policies/test.yaml --validate-only

# List entities in policy
hal-enforcer --policy policies/prod.yaml --list-entities
```

## Use Cases

### 1. Multi-Tenant SaaS Platform

Different customer workloads get isolated HAL access:
- Customer A: Full crypto + storage
- Customer B: Crypto only
- Customer C: Random + clock only

### 2. Microservice Architecture

Each microservice gets only what it needs:
- Auth Service: crypto + platform (for attestation)
- Data Service: storage + crypto
- API Gateway: sockets + communication

### 3. Security Zones

Different security levels for different zones:
- Trusted Zone: Full HAL access
- DMZ: Network + crypto only
- Untrusted: Random only

## Security Considerations

- **Policy File Protection**: Policy files should be read-only and owned by trusted authority
- **Session Management**: Sessions expire and require re-authentication
- **Audit Trail**: All operations are logged for compliance
- **Rate Limiting**: Prevents DoS and resource exhaustion
- **Capability Isolation**: Entities cannot access denied interfaces

## Development

```bash
# Build
cargo build

# Run tests
cargo test

# Run with example policy
cargo run -- --policy policies/example.yaml

# Run examples
cargo run --example basic_usage
```

## License

MIT License - see LICENSE file for details
