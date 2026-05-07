//! Emit three small contrasting WASM modules into ./demo-wasms/.
//! Run:  cargo run --example build_demo_wasms
//!
//! The modules import recognisable WASI / TEE host functions so the
//! analyser has something concrete to reason over.

use std::fs;
use std::path::Path;

const SOCKETS_APP: &str = r#"
(module
  (import "wasi_snapshot_preview1" "sock_open"   (func (param i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "sock_send"   (func (param i32 i32 i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "sock_recv"   (func (param i32 i32 i32 i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "clock_time_get" (func (param i32 i64 i32) (result i32)))
  (func (export "_start"))
)
"#;

const CRYPTO_ONLY: &str = r#"
(module
  (import "wasi_crypto_common" "array_output_pull" (func (param i32 i32 i32 i32) (result i32)))
  (import "wasi_crypto_symmetric" "symmetric_key_generate" (func (param i32 i32 i32 i32) (result i32)))
  (import "wasi_crypto_signatures" "signature_keypair_generate" (func (param i32 i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "random_get" (func (param i32 i32) (result i32)))
  (func (export "_start"))
)
"#;

const PURE_COMPUTE: &str = r#"
(module
  (func (export "add") (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.add)
)
"#;

// The "tampered" workload for Demo 2. Starts as crypto-only and adds
// a sneaky sock_open import — the kind of modification an attacker
// might slip in to exfiltrate data.
const CRYPTO_ONLY_TAMPERED: &str = r#"
(module
  (import "wasi_crypto_common" "array_output_pull" (func (param i32 i32 i32 i32) (result i32)))
  (import "wasi_crypto_symmetric" "symmetric_key_generate" (func (param i32 i32 i32 i32) (result i32)))
  (import "wasi_crypto_signatures" "signature_keypair_generate" (func (param i32 i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "random_get" (func (param i32 i32) (result i32)))
  ;; <<< malicious addition >>>
  (import "wasi_snapshot_preview1" "sock_open" (func (param i32 i32 i32) (result i32)))
  (func (export "_start"))
)
"#;

fn write(name: &str, wat_src: &str) -> anyhow::Result<()> {
    let bytes = wat::parse_str(wat_src)?;
    let dir = Path::new("demo-wasms");
    fs::create_dir_all(dir)?;
    let path = dir.join(name);
    fs::write(&path, &bytes)?;
    println!("wrote {} ({} bytes)", path.display(), bytes.len());
    Ok(())
}

fn main() -> anyhow::Result<()> {
    write("sockets-app.wasm", SOCKETS_APP)?;
    write("crypto-only.wasm", CRYPTO_ONLY)?;
    write("pure-compute.wasm", PURE_COMPUTE)?;
    write("crypto-only-tampered.wasm", CRYPTO_ONLY_TAMPERED)?;
    println!("\nNow point the analyser at each:");
    println!("  cargo run --example openai_analyzer --features openai -- demo-wasms/sockets-app.wasm");
    println!("  cargo run --example openai_analyzer --features openai -- demo-wasms/crypto-only.wasm");
    println!("  cargo run --example openai_analyzer --features openai -- demo-wasms/pure-compute.wasm");
    Ok(())
}
