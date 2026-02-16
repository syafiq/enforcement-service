# Getting Started with HAL Enforcement Service

This guide will walk you through setting up and using the HAL Enforcement Service.

## Prerequisites

- Rust 2021 edition or later
- Cargo (comes with Rust)
- Access to the ELASTIC TEE HAL library

## Step 1: Installation

### Option A: Quick Start Script

```bash
cd enforcement-service
./run.sh
```

The script will:
1. Build the service
2. Let you choose a policy file
3. Validate the policy
4. Start the service

### Option B: Manual Installation

```bash
# Build the service
cd enforcement-service
cargo build --release

# The binary will be at: target/release/hal-enforcer
```

## Step 2: Create Your Policy

Create a YAML policy file defining your entities and their capabilities:

```yaml
# my-policy.yaml
version: "1.0"

settings:
  strict_mode: true
  default_rate_limit: 100

entities:
  - id: "my-app"
    description: "My application"
    capabilities:
      crypto: true
      random: true
      clock: true
    rate_limits:
      crypto:
        operations_per_second: 1000
        burst_size: 2000
```

See [`policies/example.yaml`](policies/example.yaml) for more examples.

## Step 3: Validate Your Policy

```bash
cargo run --release -- --policy my-policy.yaml --validate-only
```

This checks:
- YAML syntax is correct
- All required fields are present
- No duplicate entity IDs
- Rate limits are valid

## Step 4: List Entities

See what entities are defined in your policy:

```bash
cargo run --release -- --policy my-policy.yaml --list-entities
```

## Step 5: Start the Service

```bash
cargo run --release -- --policy my-policy.yaml --port 8080
```

The service will start and listen on `http://localhost:8080`.

### Available Endpoints

- `GET /health` - Health check
- `POST /api/v1/hal/access` - Create HAL access session
- `GET /api/v1/hal/capabilities` - Get entity capabilities
- `GET /api/v1/entities` - List all entities
- `GET /api/v1/audit` - Get audit log
- `GET /api/v1/stats` - Service statistics

## Step 6: Use the Service

### Create a Session

```bash
curl -X POST http://localhost:8080/api/v1/hal/access \
  -H "Content-Type: application/json" \
  -d '{"entity_id": "my-app"}' | jq
```

Response:
```json
{
  "session_id": "123e4567-e89b-12d3-a456-426614174000",
  "granted_capabilities": ["crypto", "random", "clock"],
  "expires_at": "2026-02-17T12:00:00Z"
}
```

### Query Capabilities

```bash
curl http://localhost:8080/api/v1/hal/capabilities?entity_id=my-app | jq
```

### View Audit Log

```bash
curl http://localhost:8080/api/v1/audit?entity_id=my-app&limit=10 | jq
```

## Step 7: Use from Your Application

### In Rust

```rust
use hal_enforcement_service::{EnforcementService, PolicyConfig};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load policy
    let service = EnforcementService::from_file("my-policy.yaml")?;
    
    // Create session
    let session = service.create_session("my-app").await?;
    
    // Get restricted HAL
    let hal = service.create_restricted_hal("my-app").await?;
    
    // Use HAL interfaces
    if let Some(crypto) = hal.crypto {
        let hash = crypto.hash(b"data", "SHA-256")?;
        println!("Hash: {:?}", hash);
    }
    
    Ok(())
}
```

### Via HTTP API

From any language that can make HTTP requests:

```python
import requests

# Create session
response = requests.post(
    'http://localhost:8080/api/v1/hal/access',
    json={'entity_id': 'my-app'}
)
session = response.json()
print(f"Session ID: {session['session_id']}")
print(f"Capabilities: {session['granted_capabilities']}")
```

## Common Use Cases

### 1. Development Environment

```yaml
# dev-policy.yaml
version: "1.0"
settings:
  strict_mode: false  # Allow unknown entities
  default_rate_limit: 10000

entities:
  - id: "dev-app"
    capabilities:
      crypto: true
      random: true
      storage: true
      clock: true
```

### 2. Production Environment

```yaml
# prod-policy.yaml
version: "1.0"
settings:
  strict_mode: true  # Deny unknown entities
  default_rate_limit: 1000

entities:
  - id: "prod-app"
    capabilities:
      crypto: true
      storage: true
    rate_limits:
      crypto:
        operations_per_second: 1000
      storage:
        operations_per_second: 500
    quotas:
      storage:
        max_bytes: 1073741824  # 1 GB
```

### 3. Multi-Tenant Setup

```yaml
version: "1.0"
entities:
  - id: "tenant-premium"
    capabilities:
      crypto: true
      storage: true
      sockets: true
    rate_limits:
      crypto:
        operations_per_second: 5000
    quotas:
      storage:
        max_bytes: 10737418240  # 10 GB

  - id: "tenant-basic"
    capabilities:
      crypto: true
      random: true
    rate_limits:
      crypto:
        operations_per_second: 100
```

## Monitoring and Debugging

### Enable Verbose Logging

```bash
cargo run --release -- --policy my-policy.yaml --verbose
```

Or set environment variable:

```bash
RUST_LOG=debug cargo run --release -- --policy my-policy.yaml
```

### Check Active Sessions

```bash
curl http://localhost:8080/api/v1/stats | jq
```

### View Audit Trail

```bash
# All events
curl http://localhost:8080/api/v1/audit?limit=100 | jq

# For specific entity
curl http://localhost:8080/api/v1/audit?entity_id=my-app&limit=50 | jq
```

## Testing

### Run Unit Tests

```bash
cargo test
```

### Run Integration Tests

```bash
cargo test --test integration
```

### Run Example

```bash
cargo run --example basic_usage
```

## Production Deployment

### 1. Build Release Binary

```bash
cargo build --release
```

Binary will be at: `target/release/hal-enforcer`

### 2. Create Service File (systemd)

```ini
# /etc/systemd/system/hal-enforcer.service
[Unit]
Description=HAL Enforcement Service
After=network.target

[Service]
Type=simple
User=hal-enforcer
ExecStart=/usr/local/bin/hal-enforcer --policy /etc/hal-enforcer/policy.yaml --port 8080
Restart=always
RestartSec=10

[Install]
WantedBy=multi-user.target
```

### 3. Configure Log Rotation

```
# /etc/logrotate.d/hal-enforcer
/var/log/hal-enforcer/*.log {
    daily
    rotate 30
    compress
    delaycompress
    notifempty
    create 0640 hal-enforcer hal-enforcer
    sharedscripts
}
```

### 4. Security Hardening

- Run as dedicated user (not root)
- Make policy files read-only
- Enable TLS for HTTP API
- Use firewall rules to restrict access
- Monitor audit logs regularly

## Troubleshooting

### Policy Validation Fails

```bash
# Check YAML syntax
yamllint my-policy.yaml

# Validate with verbose output
cargo run -- --policy my-policy.yaml --validate-only --verbose
```

### Service Won't Start

```bash
# Check if port is already in use
lsof -i :8080

# Try different port
cargo run -- --policy my-policy.yaml --port 8081
```

### Entity Not Found

Make sure the entity ID in your request exactly matches the ID in the policy file (case-sensitive).

### Rate Limiting Issues

Check the rate_limits section in your policy. The counter resets every second.

## Next Steps

- Read [TEST_SCENARIOS.md](TEST_SCENARIOS.md) for advanced usage patterns
- Check [policies/production.yaml](policies/production.yaml) for production-ready configuration
- Review the API documentation in [README.md](README.md)

## Support

For issues and questions:
- Check the logs: `/var/log/hal-enforcer/audit.log`
- Enable debug logging: `RUST_LOG=debug`
- Review audit trail via API
