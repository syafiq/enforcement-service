# Policy Configuration Files

This directory contains YAML policy configurations for the HAL Enforcement Service.

## Policy File Structure

```yaml
version: "1.0"

# Global settings (optional)
settings:
  default_rate_limit: 100       # Default ops/sec if not specified per interface
  audit_log_path: "./logs/audit.log"
  strict_mode: true             # Deny access if entity not found

# Entity definitions
entities:
  - id: "entity-name"
    description: "Human-readable description"
    capabilities:
      platform: true/false       # Platform attestation
      capabilities: true/false   # Capability queries
      crypto: true/false         # Cryptographic operations
      random: true/false         # Random number generation
      clock: true/false          # Time operations
      storage: true/false        # Storage operations
      sockets: true/false        # Network sockets
      gpu: true/false            # GPU compute
      resources: true/false      # Resource management
      events: true/false         # Event handling
      communication: true/false  # Inter-component messaging
    
    # Optional rate limits per interface
    rate_limits:
      crypto:
        operations_per_second: 1000
        burst_size: 2000
    
    # Optional quotas per interface
    quotas:
      crypto:
        max_bytes: 10485760      # 10 MB
        max_operations: 100000
    
    # Can this entity grant capabilities to others?
    can_grant: false
```

## Example Policies

### Minimal Access (Random only)

```yaml
version: "1.0"
entities:
  - id: "minimal-app"
    capabilities:
      random: true
```

### Crypto Service

```yaml
version: "1.0"
entities:
  - id: "crypto-service"
    capabilities:
      crypto: true
      random: true
      clock: true
    rate_limits:
      crypto:
        operations_per_second: 1000
```

### Full Supervisor

```yaml
version: "1.0"
entities:
  - id: "admin"
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
    can_grant: true
```

## Validation

Validate a policy file:

```bash
hal-enforcer --policy my-policy.yaml --validate-only
```

## Best Practices

1. **Principle of Least Privilege**: Only grant capabilities that are actually needed
2. **Rate Limiting**: Always set rate limits for production workloads
3. **Quotas**: Use quotas to prevent resource exhaustion
4. **Audit Logging**: Enable audit logging for compliance
5. **Strict Mode**: Use strict mode in production to deny unknown entities
6. **Version Control**: Keep policy files in version control
7. **Documentation**: Add clear descriptions to each entity

## Security Considerations

- Policy files should be read-only in production
- Protect policy files from unauthorized modification
- Regularly audit granted capabilities
- Monitor rate limit and quota metrics
- Review audit logs for suspicious activity
