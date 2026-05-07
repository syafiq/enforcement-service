//! Workload enforcement: extract WASM imports, map them to capability
//! buckets, and verify the workload only uses what the policy granted.
//!
//! This is what makes a "policy" actually enforceable. Without it,
//! `granted_capabilities` is just metadata — the workload could still
//! import and call any host function the runtime exposes.

use crate::config::CapabilitiesConfig;
use crate::error::{EnforcementError, Result};
use wasmparser::{Parser, Payload};

/// Map a WASM import (module + name) to the capability bucket it
/// requires, or `None` if the import is benign (memory, table, etc.)
/// or not security-relevant.
pub fn import_to_capability(module: &str, name: &str) -> Option<&'static str> {
    // WASI preview1 — split by function family.
    if module == "wasi_snapshot_preview1" || module == "wasi_unstable" {
        return match name {
            // Sockets
            n if n.starts_with("sock_") => Some("sockets"),
            // Clock
            "clock_time_get" | "clock_res_get" => Some("clock"),
            // Random
            "random_get" => Some("random"),
            // Storage / filesystem
            n if n.starts_with("fd_")
                || n.starts_with("path_")
                || n == "file_open"
                || n == "file_close" =>
            {
                Some("storage")
            }
            // Process / environment — treat as platform.
            "proc_exit" | "proc_raise" | "args_get" | "args_sizes_get"
            | "environ_get" | "environ_sizes_get" => Some("platform"),
            // Polling / events
            "poll_oneoff" | "sched_yield" => Some("events"),
            _ => None,
        };
    }

    // WASI crypto family.
    if module.starts_with("wasi_crypto") || module.starts_with("wasi-crypto") {
        return Some("crypto");
    }

    // WASI sockets explicit module.
    if module.starts_with("wasi:sockets") || module == "wasi_sockets" {
        return Some("sockets");
    }

    // ELASTIC TEE HAL interfaces (matches wit-modular/*.wit names).
    match module {
        "elastic:hal/platform" | "platform" => Some("platform"),
        "elastic:hal/capabilities" => Some("capabilities"),
        "elastic:hal/crypto" | "crypto" => Some("crypto"),
        "elastic:hal/random" | "random" => Some("random"),
        "elastic:hal/clock" | "clock" => Some("clock"),
        "elastic:hal/storage" | "storage" => Some("storage"),
        "elastic:hal/sockets" | "sockets" => Some("sockets"),
        "elastic:hal/gpu" | "gpu" => Some("gpu"),
        "elastic:hal/resources" | "resources" => Some("resources"),
        "elastic:hal/events" | "events" => Some("events"),
        "elastic:hal/communication" | "communication" => Some("communication"),
        _ => None,
    }
}

/// One offending import.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    pub module: String,
    pub name: String,
    pub required: String,
}

/// Extract every `(module, name)` import from a WASM module.
pub fn extract_imports(wasm: &[u8]) -> Result<Vec<(String, String)>> {
    let mut imports = Vec::new();
    for payload in Parser::new(0).parse_all(wasm) {
        let payload = payload
            .map_err(|e| EnforcementError::Policy(format!("invalid WASM: {e}")))?;
        if let Payload::ImportSection(section) = payload {
            for imp in section {
                let imp = imp
                    .map_err(|e| EnforcementError::Policy(format!("bad import: {e}")))?;
                imports.push((imp.module.to_string(), imp.name.to_string()));
            }
        }
    }
    Ok(imports)
}

/// Verify that every WASM import maps to a capability the policy granted.
/// Returns `Err(EnforcementError::PolicyViolation)` listing the first
/// violation; on success returns the list of (import, bucket) pairs that
/// were checked, useful for audit.
pub fn check_workload(
    wasm: &[u8],
    granted: &CapabilitiesConfig,
) -> Result<Vec<(String, String, String)>> {
    let imports = extract_imports(wasm)?;
    let mut checked = Vec::with_capacity(imports.len());

    for (module, name) in imports {
        let Some(bucket) = import_to_capability(&module, &name) else {
            // Unknown / benign import — let it through. (Memory, tables,
            // user-defined modules, etc.)
            continue;
        };
        if !granted.has_capability(bucket) {
            return Err(EnforcementError::Policy(format!(
                "workload imports `{module}::{name}` which requires capability \
                 `{bucket}`, but the policy did not grant it"
            )));
        }
        checked.push((module, name, bucket.to_string()));
    }
    Ok(checked)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn caps_with(list: &[&str]) -> CapabilitiesConfig {
        let mut c = CapabilitiesConfig::default();
        for s in list {
            match *s {
                "platform" => c.platform = true,
                "capabilities" => c.capabilities = true,
                "crypto" => c.crypto = true,
                "random" => c.random = true,
                "clock" => c.clock = true,
                "storage" => c.storage = true,
                "sockets" => c.sockets = true,
                "gpu" => c.gpu = true,
                "resources" => c.resources = true,
                "events" => c.events = true,
                "communication" => c.communication = true,
                _ => panic!("unknown cap"),
            }
        }
        c
    }

    #[test]
    fn maps_known_imports() {
        assert_eq!(
            import_to_capability("wasi_snapshot_preview1", "sock_open"),
            Some("sockets")
        );
        assert_eq!(
            import_to_capability("wasi_snapshot_preview1", "random_get"),
            Some("random")
        );
        assert_eq!(
            import_to_capability("wasi_crypto_symmetric", "symmetric_key_generate"),
            Some("crypto")
        );
    }

    #[test]
    fn unknown_imports_are_ignored() {
        assert_eq!(import_to_capability("env", "memory"), None);
        assert_eq!(import_to_capability("user_module", "do_thing"), None);
    }

    #[test]
    fn check_passes_when_all_buckets_granted() {
        // (module
        //   (import "wasi_snapshot_preview1" "random_get" (func (param i32 i32) (result i32))))
        let wasm = wat::parse_str(
            r#"(module (import "wasi_snapshot_preview1" "random_get"
                       (func (param i32 i32) (result i32))))"#,
        )
        .unwrap();
        let granted = caps_with(&["random"]);
        check_workload(&wasm, &granted).unwrap();
    }

    #[test]
    fn check_fails_when_bucket_missing() {
        let wasm = wat::parse_str(
            r#"(module (import "wasi_snapshot_preview1" "sock_open"
                       (func (param i32 i32 i32) (result i32))))"#,
        )
        .unwrap();
        let granted = caps_with(&["random"]); // no sockets
        let err = check_workload(&wasm, &granted).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("sock_open"));
        assert!(msg.contains("sockets"));
    }
}
