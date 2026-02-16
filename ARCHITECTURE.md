# HAL Enforcement Service - Architecture Overview

## System Architecture

```
┌──────────────────────────────────────────────────────────────────┐
│                    Centralized Policy Authority                 │
│              (YAML Configuration Management)                    │
└──────────────────────────────────────────────────────────────────┘
                              │
                              │ YAML Policy Files
                              ▼
┌──────────────────────────────────────────────────────────────────┐
│              HAL Enforcement Service (Port 8080)                │
│                                                                  │
│  ┌────────────────────────────────────────────────────────┐    │
│  │ Policy Engine                                           │    │
│  │  - Load & validate YAML policies                       │    │
│  │  - Manage entity configurations                        │    │
│  │  - Enforce capability restrictions                     │    │
│  └────────────────────────────────────────────────────────┘    │
│                                                                  │
│  ┌────────────────────────────────────────────────────────┐    │
│  │ Session Manager                                         │    │
│  │  - Create/manage entity sessions (UUID-based)          │    │
│  │  - Track session expiration (24h default)              │    │
│  │  - Monitor operation counts per session                │    │
│  └────────────────────────────────────────────────────────┘    │
│                                                                  │
│  ┌────────────────────────────────────────────────────────┐    │
│  │ Rate Limiter                                            │    │
│  │  - Per-entity, per-interface rate limits               │    │
│  │  - Token bucket algorithm                              │    │
│  │  - Configurable burst sizes                            │    │
│  └────────────────────────────────────────────────────────┘    │
│                                                                  │
│  ┌────────────────────────────────────────────────────────┐    │
│  │ Quota Tracker                                           │    │
│  │  - Track data usage (max_bytes)                        │    │
│  │  - Track operation counts (max_operations)             │    │
│  │  - Per-entity quota enforcement                        │    │
│  └────────────────────────────────────────────────────────┘    │
│                                                                  │
│  ┌────────────────────────────────────────────────────────┐    │
│  │ Audit Logger                                            │    │
│  │  - Log all HAL access attempts                         │    │
│  │  - Timestamp, entity, operation, result                │    │
│  │  - Queryable via API                                   │    │
│  └────────────────────────────────────────────────────────┘    │
│                                                                  │
│  ┌────────────────────────────────────────────────────────┐    │
│  │ HTTP API Server (Axum)                                  │    │
│  │  - /api/v1/hal/access         - Create sessions        │    │
│  │  - /api/v1/hal/capabilities   - Query capabilities     │    │
│  │  - /api/v1/entities           - List entities          │    │
│  │  - /api/v1/audit              - Audit log access       │    │
│  │  - /api/v1/stats              - Service statistics     │    │
│  └────────────────────────────────────────────────────────┘    │
│                                                                  │
│  ┌────────────────────────────────────────────────────────┐    │
│  │ HAL Provider Factory                                    │    │
│  │  - Create restricted HalProvider instances             │    │
│  │  - Only include granted interfaces                     │    │
│  │  - Wrap with rate limiting/quota checking              │    │
│  └────────────────────────────────────────────────────────┘    │
└──────────────────────────────────────────────────────────────────┘
                              │
                              │ Restricted HAL Access
                              ▼
┌──────────────────────────────────────────────────────────────────┐
│                         WASM Components                         │
│                                                                  │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐          │
│  │ crypto-worker│  │ attestation  │  │ untrusted-app│          │
│  │              │  │              │  │              │          │
│  │ Caps:        │  │ Caps:        │  │ Caps:        │          │
│  │ • crypto     │  │ • platform   │  │ • random     │          │
│  │ • random     │  │ • caps       │  │ • clock      │          │
│  │ • clock      │  │              │  │              │          │
│  └──────────────┘  └──────────────┘  └──────────────┘          │
└──────────────────────────────────────────────────────────────────┘
                              │
                              │ Via HalProvider trait
                              ▼
┌──────────────────────────────────────────────────────────────────┐
│                      ELASTIC TEE HAL                            │
│                                                                  │
│  Platform │ Capabilities │ Crypto │ Random │ Clock │ Storage   │
│  Sockets  │ GPU          │ Resources │ Events │ Communication  │
└──────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌──────────────────────────────────────────────────────────────────┐
│                  TEE Hardware (AMD SEV-SNP / Intel TDX)        │
└──────────────────────────────────────────────────────────────────┘
```

## Component Breakdown

### 1. Configuration Layer (YAML Policies)

**Files:**
- `policies/example.yaml` - Development examples
- `policies/production.yaml` - Production-ready config
- `policies/README.md` - Policy documentation

**Responsibilities:**
- Define entities and their identities
- Specify granted capabilities per entity
- Configure rate limits and quotas
- Set global enforcement settings

### 2. Service Core (`src/`)

#### `config.rs` - Policy Configuration
- Load and parse YAML policies
- Validate policy structure
- Query entity configurations
- Provide type-safe access to settings

#### `service.rs` - Enforcement Engine
- Manage entity sessions (create, retrieve, expire)
- Create restricted HAL providers
- Check capabilities and permissions
- Enforce rate limits
- Track quotas
- Audit logging

#### `api.rs` - HTTP API
- REST endpoints for HAL access
- Session management API
- Audit log querying
- Service metrics

#### `error.rs` - Error Handling
- Unified error types
- HTTP status code mapping
- Detailed error messages

### 3. API Endpoints

| Method | Endpoint | Purpose |
|--------|----------|---------|
| GET | `/health` | Service health check |
| POST | `/api/v1/hal/access` | Create HAL access session |
| GET | `/api/v1/hal/capabilities` | Query entity capabilities |
| GET | `/api/v1/entities` | List all defined entities |
| GET | `/api/v1/audit` | Retrieve audit logs |
| GET | `/api/v1/stats` | Service statistics |

### 4. Integration with WASMHAL

```rust
// Service creates restricted HAL provider
let hal = service.create_restricted_hal("entity-id").await?;

// HalProvider contains only granted interfaces
pub struct HalProvider {
    pub platform: Option<Box<dyn PlatformInterface>>,
    pub capabilities: Option<Box<dyn CapabilitiesInterface>>,
    pub crypto: Option<Box<dyn CryptoInterface>>,
    pub random: Option<Box<dyn RandomInterface>>,
    pub clock: Option<Box<dyn ClockInterface>>,
    pub storage: Option<Box<dyn StorageInterface>>,
    // ... other interfaces
}

// WASM component uses only what's granted
if let Some(crypto) = hal.crypto {
    crypto.hash(data, "SHA-256")?;
}
```

## Data Flow

### Session Creation Flow

```
1. Client Request
   POST /api/v1/hal/access
   { "entity_id": "crypto-worker" }
   
2. Service validates entity exists in policy
   
3. Service creates session
   - Generate UUID
   - Set 24h expiration
   - Record granted capabilities
   
4. Service responds
   {
     "session_id": "uuid",
     "granted_capabilities": ["crypto", "random"],
     "expires_at": "2026-02-17T12:00:00Z"
   }
```

### HAL Access Flow

```
1. Service receives HAL access request for entity
   
2. Service loads entity policy
   
3. Service creates HalProvider with:
   - Only granted interfaces instantiated
   - Others set to None
   
4. Rate limiting wrapper applied
   
5. Quota tracking wrapper applied
   
6. Audit logging wrapper applied
   
7. Return restricted HAL to client
```

### Operation Execution Flow

```
1. Client calls HAL interface method
   hal.crypto.hash(data, "SHA-256")
   
2. Audit logger records attempt
   
3. Rate limiter checks limit
   - If exceeded: return RateLimitExceeded error
   - If OK: proceed
   
4. Quota tracker checks quota
   - If exceeded: return QuotaExceeded error
   - If OK: proceed
   
5. Actual HAL operation executes
   
6. Audit logger records result
   
7. Return result to client
```

## Security Model

### Principle of Least Privilege
- Entities only get capabilities they need
- Default deny for non-granted interfaces
- Explicit capability grants required

### Defense in Depth
1. **Policy Layer**: YAML configuration validation
2. **Service Layer**: Runtime permission checks
3. **Rate Limiting**: Prevent abuse
4. **Quota Enforcement**: Resource limits
5. **Audit Logging**: Complete trail

### Isolation Boundaries
```
┌─────────────────────────────────────────┐
│ untrusted-app                           │
│ Capabilities: [random, clock]           │
│ CANNOT access: crypto, storage, sockets │
└─────────────────────────────────────────┘

┌─────────────────────────────────────────┐
│ crypto-worker                           │
│ Capabilities: [crypto, random, clock]   │
│ CANNOT access: storage, sockets, gpu    │
└─────────────────────────────────────────┘
```

## Deployment Models

### Standalone Service
```
enforcement-service (Port 8080)
    ↓
HAL access via HTTP API
    ↓
WASM components connect remotely
```

### Embedded Library
```rust
use hal_enforcement_service::EnforcementService;

let service = EnforcementService::from_file("policy.yaml")?;
let hal = service.create_restricted_hal("entity-id").await?;
// Use hal directly
```

### Multi-Tenant Platform
```
                    Enforcement Service
                           │
        ┌──────────────────┼──────────────────┐
        │                  │                  │
    Tenant A           Tenant B           Tenant C
    (crypto+storage)   (crypto only)      (random only)
```

## Directory Structure

```
enforcement-service/
├── Cargo.toml              # Dependencies and build config
├── README.md               # Main documentation
├── GETTING_STARTED.md      # Quick start guide
├── TEST_SCENARIOS.md       # Usage scenarios
├── ARCHITECTURE.md         # This file
├── .gitignore
├── run.sh                  # Quick start script
│
├── src/
│   ├── lib.rs             # Library exports
│   ├── main.rs            # CLI entry point
│   ├── config.rs          # Policy loading
│   ├── service.rs         # Enforcement engine
│   ├── api.rs             # HTTP endpoints
│   └── error.rs           # Error types
│
├── policies/
│   ├── example.yaml       # Development policy
│   ├── production.yaml    # Production template
│   └── README.md          # Policy documentation
│
├── examples/
│   └── basic_usage.rs     # Usage example
│
└── tests/
    └── integration.rs     # Integration tests
```

## Extension Points

### Custom Providers
Implement custom HAL providers with additional enforcement logic:

```rust
struct CustomCryptoProvider {
    inner: DefaultCryptoProvider,
    custom_logic: MyLogic,
}

impl CryptoInterface for CustomCryptoProvider {
    fn hash(&self, data: &[u8], algo: &str) -> Result<Vec<u8>, String> {
        self.custom_logic.pre_check()?;
        self.inner.hash(data, algo)
    }
}
```

### Additional Metrics
Add custom metrics collection:

```rust
service.on_operation(|entity, interface, op, result| {
    metrics::record(entity, interface, op, result);
});
```

### Policy Hot Reload
Watch policy file for changes:

```rust
service.watch_policy("policy.yaml", |new_policy| {
    service.reload_policy(new_policy)?;
});
```

## Performance Characteristics

### Operation Latency
- Session creation: <1ms
- HAL access creation: <10ms
- Rate limit check: <100μs
- Audit log write: <500μs (async)

### Memory Usage
- Base service: ~10MB
- Per session: ~1KB
- Per HAL provider: ~100KB

### Throughput
- Session creation: >10,000/sec
- HAL operations: >100,000/sec (depends on HAL impl)
- Rate limit checks: >1,000,000/sec

## Future Enhancements

1. **Dynamic Policy Updates**: Hot reload without restart
2. **Distributed Deployment**: Multiple enforcement service instances
3. **Policy Versioning**: Track and rollback policy changes
4. **Advanced Quotas**: Time-based quotas (daily, monthly)
5. **Capability Delegation**: Entities granting capabilities to others
6. **WebAssembly Integration**: Direct WASM component integration
7. **Metrics Dashboard**: Real-time monitoring UI
8. **Policy Templates**: Pre-built policy patterns
9. **Compliance Reports**: Automated compliance reporting
10. **mTLS Support**: Mutual TLS for service-to-service auth
