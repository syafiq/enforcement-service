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

### 3. Request HAL Access

A HAL runtime asks the service which HAL interfaces an entity may use:

```bash
curl -s -X POST http://localhost:8080/api/v1/hal/access \
  -H 'Content-Type: application/json' \
  -d '{"entity_id": "crypto-worker"}'
# {"session_id":"…","granted_capabilities":["crypto","random","clock"],"expires_at":"…"}
```

When the service is embedded as a library, it can also admit a Wasm workload. In that case the module's imports are checked against the entity's capabilities, and the module is rejected if it imports a denied interface:

```rust
let service = EnforcementService::from_file("policies/my-policies.yaml")?;
let wasm = std::fs::read("app.wasm")?;
let session = service.create_session_with_wasm("crypto-worker", Some(&wasm)).await?;
```

[WasmHAL](https://github.com/elasticproject-eu/wasmhal)'s runtime and enforcement layer then link and enforce only `session.granted_capabilities`. There is no separate client library; any HTTP client can call the API.

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

| Method and path | Purpose |
| --- | --- |
| `GET /health` | Liveness check: `{"status":"healthy","service":…,"version":…}` |
| `POST /api/v1/hal/access` | Create a session for an entity and return its granted capabilities |
| `GET /api/v1/hal/capabilities?entity_id=<id>` | An entity's capabilities, rate limits and quotas |
| `GET /api/v1/entities` | The entities in the loaded policy |
| `GET /api/v1/audit?entity_id=<id>&limit=100` | Recent audit events, optionally for one entity |
| `GET /api/v1/stats` | Active sessions, number of entities, version |

### Request HAL Access

```
POST /api/v1/hal/access
Content-Type: application/json

{ "entity_id": "crypto-worker" }

Response:
{
  "session_id": "uuid-v4",
  "granted_capabilities": ["crypto", "random", "clock"],
  "expires_at": "2026-02-16T12:00:00Z"
}
```

Unknown entities get `404`. Sessions expire after 24 hours.

## CLI Usage

```bash
# Start service with policy file
hal-enforcer --policy policies/production.yaml --port 8080

# Enable verbose logging
hal-enforcer --policy policies/dev.yaml --verbose

# Choose the port (default 8080)
hal-enforcer --policy policies/prod.yaml --port 9090

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

- **Least privilege:** each entity gets only the HAL interfaces listed in its policy, and Wasm workloads that import other interfaces are rejected at admission.
- **Network access:** run the service where only the HAL runtime can reach its port.
- **Policy file:** keep it read-only and owned by the policy authority. Validate changes with `--validate-only`.
- **Audit log:** events are kept in memory, and are lost on restart.

See [SECURITY.md](SECURITY.md) for the full deployment guidelines and how to report vulnerabilities.

## Deployment

The service is a single, self-contained binary (`hal-enforcer`). It needs no TEE hardware and no root privileges. It is usually deployed next to the HAL runtime, inside the same Confidential VM.

**Requirements:**
- Rust (stable) to build.
- A C linker (Debian/Ubuntu: `sudo apt-get install -y build-essential`).
- Tested on Ubuntu 24.04, including inside AMD SEV-SNP and Intel TDX Confidential VMs on GCP. See [PERFORMANCE.md](PERFORMANCE.md).

**Build and install:**

```bash
cargo build --release                # binary: target/release/hal-enforcer
sudo install -m 0755 target/release/hal-enforcer /usr/local/bin/
```

**Run as a systemd service** (example):

```ini
# /etc/systemd/system/hal-enforcer.service
[Unit]
Description=ELASTIC HAL Enforcement Service
After=network.target

[Service]
User=hal-enforcer
ExecStart=/usr/local/bin/hal-enforcer --policy /etc/hal-enforcer/policy.yaml --port 8080
Restart=on-failure
NoNewPrivileges=true
ProtectSystem=strict

[Install]
WantedBy=multi-user.target
```

Restrict access to the port, for example with a host firewall, so that only the HAL runtime can reach it. Policies are read at start-up, so restart the service after changing the policy.

## Troubleshooting and debugging

### Logging

```bash
hal-enforcer --policy policy.yaml --verbose       # debug-level logging
RUST_LOG=hal_enforcement_service=trace hal-enforcer --policy policy.yaml
```

`--verbose` sets the default log level to `debug`. `RUST_LOG` takes precedence, if set.

### Checking a policy and the running service

```bash
hal-enforcer --policy policy.yaml --validate-only   # parse and validate, then exit
hal-enforcer --policy policy.yaml --list-entities   # show entities and their capabilities
curl -s localhost:8080/health                       # is the service up?
curl -s localhost:8080/api/v1/stats                 # active sessions and entity count
curl -s 'localhost:8080/api/v1/audit?limit=20'      # recent audit events
```

### Common problems

| Symptom | Cause and fix |
| --- | --- |
| Start-up fails with a YAML or policy error | Run `--validate-only` to see the exact field. Check the indentation and the capability names (`platform`, `capabilities`, `crypto`, `random`, `clock`, `storage`, `sockets`, `gpu`, `resources`, `events`, `communication`). |
| `Address already in use` | Another process holds the port. Choose another with `--port`. |
| `POST /api/v1/hal/access` returns `404 {"error":"<entity id>"}` | The `entity_id` isn't in the loaded policy. Check it with `--list-entities`, and restart the service after editing the policy. |
| `500 … is in auto mode but no WASM bytes were supplied` | Auto mode needs the Wasm bytes, which the HTTP API does not take. Use `manual` mode, or embed the service as a library. |
| ``workload imports `…` which requires capability `…`, but the policy did not grant it`` | The workload imports a HAL interface its entity isn't granted. Grant that capability, or remove the import. |
| A rate-limit error | The entity exceeded `operations_per_second`/`burst_size` for that interface. Adjust `rate_limits` in the policy. |

## Performance

Measured on GCP Confidential VMs (AMD SEV-SNP and Intel TDX):

- **Checks on every HAL call:** a capability check takes 8 ns and a rate-limit check takes 0.26–0.31 µs.
- **Admitting a workload** (import extraction, policy check and session creation) takes 3–3.5 µs for a small module and 211–241 µs for a 4.2 MiB module with 200 imports.
- **Rejecting a non-compliant workload** takes about 1.2 µs.
- **HTTP API throughput:** about 23,000 (SEV-SNP) to 36,000 (TDX) session requests per second with 32 concurrent clients.

See [PERFORMANCE.md](PERFORMANCE.md) for the full results, the method, and how to reproduce them.

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
cargo run --release --example perf     # benchmarks, see PERFORMANCE.md
```

## Contributing

Contributions are welcome. See [CONTRIBUTING.md](CONTRIBUTING.md). Everyone taking part is expected to follow the [Code of Conduct](CODE_OF_CONDUCT.md). Please report security vulnerabilities privately, as described in [SECURITY.md](SECURITY.md).

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Funding

This work has been partially supported by the [ELASTIC project](https://elasticproject.eu/), which received funding from the [Smart Networks and Services Joint Undertaking](https://smart-networks.europa.eu/) (SNS JU) under the European Union’s [Horizon Europe](https://research-and-innovation.ec.europa.eu/funding/funding-opportunities/funding-programmes-and-open-calls/horizon-europe_en) research and innovation programme under [Grant Agreement No. 101139067](https://cordis.europa.eu/project/id/101139067). Views and opinions expressed are however those of the author(s) only and do not necessarily reflect those of the European Union. Neither the European Union nor the granting authority can be held responsible for them.
