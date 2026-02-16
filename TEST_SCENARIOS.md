# HAL Enforcement Service - Test Scenarios

This document describes various test scenarios for the enforcement service.

## Scenario 1: Multi-Tenant SaaS

Different customers get isolated HAL access with varying privilege levels.

```yaml
version: "1.0"
entities:
  - id: "tenant-gold-001"
    description: "Gold tier customer - full crypto + storage"
    capabilities:
      crypto: true
      random: true
      storage: true
      clock: true
    rate_limits:
      crypto:
        operations_per_second: 5000
      storage:
        operations_per_second: 2000
    quotas:
      storage:
        max_bytes: 10737418240  # 10 GB

  - id: "tenant-silver-002"
    description: "Silver tier customer - crypto only"
    capabilities:
      crypto: true
      random: true
      clock: true
    rate_limits:
      crypto:
        operations_per_second: 1000
    quotas:
      crypto:
        max_operations: 100000

  - id: "tenant-bronze-003"
    description: "Bronze tier customer - basic access"
    capabilities:
      random: true
      clock: true
    rate_limits:
      random:
        operations_per_second: 100
```

## Scenario 2: Microservice Architecture

Each microservice gets exactly what it needs:

```yaml
version: "1.0"
entities:
  - id: "auth-service"
    description: "Authentication and authorization"
    capabilities:
      crypto: true
      platform: true  # For attestation
      random: true
      storage: true
      clock: true

  - id: "api-gateway"
    description: "External API gateway"
    capabilities:
      sockets: true
      crypto: true  # For TLS
      communication: true
      clock: true

  - id: "data-service"
    description: "Data persistence layer"
    capabilities:
      storage: true
      crypto: true
      clock: true

  - id: "worker-service"
    description: "Background job processor"
    capabilities:
      events: true
      crypto: true
      storage: true
      clock: true
```

## Scenario 3: Security Zones

Different zones with different trust levels:

```yaml
version: "1.0"
entities:
  - id: "trusted-zone-app"
    description: "Application in trusted security zone"
    capabilities:
      platform: true
      crypto: true
      random: true
      storage: true
      sockets: true
      clock: true

  - id: "dmz-zone-app"
    description: "DMZ application (limited trust)"
    capabilities:
      sockets: true
      crypto: true
      clock: true
    rate_limits:
      sockets:
        operations_per_second: 1000

  - id: "untrusted-zone-app"
    description: "Untrusted application (minimal access)"
    capabilities:
      random: true
      clock: true
    rate_limits:
      random:
        operations_per_second: 50
```

## Scenario 4: Development vs Production

Development environment with relaxed limits:

```yaml
# development.yaml
version: "1.0"
settings:
  strict_mode: false  # Allow unknown entities
  default_rate_limit: 10000

entities:
  - id: "dev-app"
    capabilities:
      platform: true
      crypto: true
      random: true
      storage: true
      clock: true
```

Production environment with strict controls:

```yaml
# production.yaml
version: "1.0"
settings:
  strict_mode: true  # Deny unknown entities
  default_rate_limit: 1000

entities:
  - id: "prod-app"
    capabilities:
      crypto: true
      storage: true
      clock: true
    rate_limits:
      crypto:
        operations_per_second: 1000
      storage:
        operations_per_second: 500
    quotas:
      storage:
        max_bytes: 1073741824
```

## Testing API Endpoints

### Create HAL Access

```bash
curl -X POST http://localhost:8080/api/v1/hal/access \
  -H "Content-Type: application/json" \
  -d '{"entity_id": "crypto-worker"}'
```

### Get Entity Capabilities

```bash
curl http://localhost:8080/api/v1/hal/capabilities?entity_id=crypto-worker
```

### List Entities

```bash
curl http://localhost:8080/api/v1/entities
```

### Get Audit Log

```bash
curl http://localhost:8080/api/v1/audit?entity_id=crypto-worker&limit=50
```

### Get Service Stats

```bash
curl http://localhost:8080/api/v1/stats
```

## Load Testing

Use Apache Bench to test rate limiting:

```bash
# Test rate limiting for crypto operations
ab -n 1000 -c 10 -p request.json -T application/json \
  http://localhost:8080/api/v1/hal/access
```

## Integration Testing

```bash
# Run integration tests
cargo test --test integration

# Run with logging
RUST_LOG=debug cargo test --test integration -- --nocapture
```
