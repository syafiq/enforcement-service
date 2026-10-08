# Security Policy

## Reporting a vulnerability

Please **do not report security vulnerabilities in public GitHub issues.**

Report them privately through [GitHub private vulnerability reporting](https://github.com/syafiq/enforcement-service/security/advisories/new). Please include:

- the commit;
- the policy file, or a minimal version of it;
- the request or workload that triggers the problem, and the impact you expect.

We aim to acknowledge reports within 5 working days. We'll agree a disclosure timeline with you, and credit you in the advisory unless you prefer otherwise.

## Supported versions

This is research software developed within the ELASTIC project. Security fixes are made on the `main` branch only.

## Security model

The Enforcement Service decides **which HAL interfaces each entity may use**:

- Policies are loaded from a YAML file.
- At admission, the service checks that a Wasm workload imports only the HAL interfaces granted to its entity.
- It creates sessions that list the granted capabilities.
- It applies per-entity rate limits and records audit events.

The HAL runtime and its enforcement layer ([WasmHAL](https://github.com/elasticproject-eu/wasmhal)) then link and enforce only those interfaces.

The service trusts:

- **the policy file**: whoever can change it controls every entity's capabilities;
- **the host it runs on**, and the HAL runtime that calls its HTTP API;
- **the analyser behind `auto` mode**, when one is configured.

## Guidelines for deploying the service

### Limit network access

Run the service where only the HAL runtime can reach its port, for example inside the same Confidential VM. Restrict the port with a host firewall or network policy, and don't expose it to the internet.

### The `openai` feature sends workload code outside the TEE

With `--features openai`, `OpenAiAnalyzer` sends the **decrypted Wasm bytes** of the workload to an external API (`api.openai.com`). Don't use it for confidential workloads, unless sending the code to that provider is acceptable.

### Protect the policy file

- Make the policy file read-only, owned by the policy authority, and not writable by the service user.
- Check changes before deploying them, with `hal-enforcer --policy <file> --validate-only`.
- Policies are loaded at start-up only, so restart the service to apply a change.

### Run with least privilege

The service needs no root privileges and no TEE devices. Run it as an unprivileged user.

### The audit log is in memory

- Audit events are kept in memory only. They are lost on restart, and they grow while the service runs.
- The `--audit-log` option is accepted, but it does not write a file yet.
- If you need a durable audit trail, collect the events through `GET /api/v1/audit`, and restart long-running instances periodically.

### Policy notes

- Unknown entities are always denied. The `strict_mode` setting currently has no effect.
- Sessions expire after 24 hours.

### Dependencies

Keep the Rust toolchain and dependencies up to date. Check them regularly with [`cargo audit`](https://github.com/rustsec/rustsec/tree/main/cargo-audit).

## Scope

In scope:
- this repository's code: the service, its HTTP API and policy handling, and the example policies, where they show unsafe configuration.

Out of scope:
- vulnerabilities in WasmHAL (please report those in [its repository](https://github.com/elasticproject-eu/wasmhal/security));
- vulnerabilities in third-party crates, the operating system, or an external analyser provider.
