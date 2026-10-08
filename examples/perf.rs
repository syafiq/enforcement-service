//! Micro-benchmarks for the HAL Enforcement Service.
//!
//! Measures the per-call cost of the enforcement checks, the cost of
//! admitting a WASM workload (import extraction + policy check + session
//! creation), policy loading, and the HTTP API round trip.
//!
//! Run (release mode is required for meaningful numbers):
//!     cargo run --release --example perf
//!
//! Output is a Markdown table suitable for PERFORMANCE.md.

use hal_enforcement_service::{
    api::create_router, check_workload, extract_imports, EnforcementService, PolicyConfig,
};
use std::hint::black_box;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const SAMPLES: usize = 30;
const TARGET_SAMPLE: Duration = Duration::from_millis(20);

struct Stat {
    median: Duration,
    p95: Duration,
}

/// Time `f` and return per-operation median and p95 over `SAMPLES` batches.
/// The batch size is calibrated so each batch takes roughly `TARGET_SAMPLE`.
fn bench<F: FnMut()>(mut f: F) -> Stat {
    let mut batch = 1usize;
    loop {
        let t = Instant::now();
        for _ in 0..batch {
            f();
        }
        let el = t.elapsed();
        if el >= TARGET_SAMPLE / 4 || batch >= 1 << 24 {
            let per = el.as_secs_f64() / batch as f64;
            batch = ((TARGET_SAMPLE.as_secs_f64() / per.max(1e-9)) as usize).clamp(1, 1 << 24);
            break;
        }
        batch *= 2;
    }
    let mut per_op: Vec<f64> = (0..SAMPLES)
        .map(|_| {
            let t = Instant::now();
            for _ in 0..batch {
                f();
            }
            t.elapsed().as_secs_f64() / batch as f64
        })
        .collect();
    per_op.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let pick = |q: f64| Duration::from_secs_f64(per_op[((per_op.len() - 1) as f64 * q) as usize]);
    Stat {
        median: pick(0.5),
        p95: pick(0.95),
    }
}

fn fmt(d: Duration) -> String {
    let ns = d.as_secs_f64() * 1e9;
    if ns < 1_000.0 {
        format!("{ns:.0} ns")
    } else if ns < 1_000_000.0 {
        format!("{:.2} µs", ns / 1e3)
    } else {
        format!("{:.2} ms", ns / 1e6)
    }
}

fn row(op: &str, s: &Stat) {
    println!("| {op} | {} | {} |", fmt(s.median), fmt(s.p95));
}

fn header(title: &str) {
    println!("\n### {title}\n");
    println!("| Operation | Median | p95 |");
    println!("| --- | ---: | ---: |");
}

const POLICY: &str = r#"
version: "1.0"
settings:
  default_rate_limit: 1000000000000
  strict_mode: true
entities:
  - id: "bench"
    description: "Benchmark entity with full access"
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
  - id: "restricted"
    description: "Entity with crypto only"
    capabilities:
      crypto: true
"#;

/// Imports a typical WASI / ELASTIC HAL workload might use.
const IMPORT_POOL: &[(&str, &str, &str)] = &[
    (
        "wasi_snapshot_preview1",
        "fd_write",
        "(param i32 i32 i32 i32) (result i32)",
    ),
    (
        "wasi_snapshot_preview1",
        "fd_read",
        "(param i32 i32 i32 i32) (result i32)",
    ),
    (
        "wasi_snapshot_preview1",
        "path_open",
        "(param i32 i32 i32 i32 i32 i64 i64 i32 i32) (result i32)",
    ),
    (
        "wasi_snapshot_preview1",
        "clock_time_get",
        "(param i32 i64 i32) (result i32)",
    ),
    (
        "wasi_snapshot_preview1",
        "random_get",
        "(param i32 i32) (result i32)",
    ),
    (
        "wasi_snapshot_preview1",
        "sock_send",
        "(param i32 i32 i32 i32 i32) (result i32)",
    ),
    ("wasi_snapshot_preview1", "proc_exit", "(param i32)"),
    (
        "elastic:hal/crypto",
        "hash",
        "(param i32 i32 i32) (result i32)",
    ),
    (
        "elastic:hal/platform",
        "attestation",
        "(param i32 i32) (result i32)",
    ),
    ("env", "memcpy", "(param i32 i32 i32) (result i32)"),
];

/// Build a synthetic module with `n_imports` imports, `n_funcs` small
/// functions and a data segment of `data_bytes`, to vary module size.
fn synth_module(n_imports: usize, n_funcs: usize, data_bytes: usize) -> Vec<u8> {
    let mut w = String::from("(module\n");
    for i in 0..n_imports {
        let (m, n, sig) = IMPORT_POOL[i % IMPORT_POOL.len()];
        // Suffix keeps names unique for the "env" bucket; WASI names must stay exact.
        let name = if m == "env" {
            format!("{n}_{i}")
        } else {
            n.to_string()
        };
        w.push_str(&format!("  (import \"{m}\" \"{name}\" (func {sig}))\n"));
    }
    w.push_str("  (memory 1)\n");
    for i in 0..n_funcs {
        w.push_str(&format!(
            "  (func $f{i} (param i32) (result i32) local.get 0 i32.const {i} i32.add i32.const 3 i32.mul)\n"
        ));
    }
    if data_bytes > 0 {
        w.push_str(&format!(
            "  (data (i32.const 0) \"{}\")\n",
            "a".repeat(data_bytes)
        ));
    }
    w.push_str("  (func (export \"_start\"))\n)\n");
    wat::parse_str(&w).expect("valid wat")
}

async fn http_post(addr: std::net::SocketAddr, path: &str, body: &str) -> usize {
    let mut s = tokio::net::TcpStream::connect(addr).await.unwrap();
    s.set_nodelay(true).unwrap();
    let req = format!(
        "POST {path} HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    s.write_all(req.as_bytes()).await.unwrap();
    let mut buf = Vec::with_capacity(512);
    s.read_to_end(&mut buf).await.unwrap();
    assert!(
        buf.starts_with(b"HTTP/1.1 200"),
        "{}",
        String::from_utf8_lossy(&buf)
    );
    buf.len()
}

fn main() -> anyhow::Result<()> {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let policy = PolicyConfig::from_yaml(POLICY)?;
    let svc = Arc::new(EnforcementService::new(policy.clone())?);
    let granted = policy.find_entity("bench").unwrap().capabilities.clone();

    // ------------------------------------------------------------------
    // 1. Per-call enforcement checks
    // ------------------------------------------------------------------
    header("Per-call enforcement checks");
    row(
        "Capability check (`has_capability`)",
        &bench(|| {
            black_box(svc.has_capability(black_box("bench"), "crypto").unwrap());
        }),
    );
    row(
        "Rate-limit check (`check_rate_limit`)",
        &bench(|| {
            rt.block_on(svc.check_rate_limit(black_box("bench"), "crypto"))
                .unwrap()
        }),
    );
    let sid = rt.block_on(svc.create_session("bench"))?.session_id;
    row(
        "Session lookup (`get_session`)",
        &bench(|| {
            black_box(rt.block_on(svc.get_session(black_box(sid))).unwrap());
        }),
    );
    row(
        "Audit event (`audit`)",
        &bench(|| rt.block_on(svc.audit("bench", sid, "crypto", "hash", true, None))),
    );

    // ------------------------------------------------------------------
    // 2. Workload admission
    // ------------------------------------------------------------------
    println!("\n### Workload admission (import extraction + policy check + session)\n");
    println!("| Workload | Size | Imports | Import extraction | Policy check | Full admission (`create_session_with_wasm`) |");
    println!("| --- | ---: | ---: | ---: | ---: | ---: |");
    for (label, imports, funcs, data) in [
        ("Small", 5usize, 10usize, 0usize),
        ("Medium", 50, 1_000, 256 * 1024),
        ("Large", 200, 10_000, 4 * 1024 * 1024),
    ] {
        let wasm = synth_module(imports, funcs, data);
        let ex = bench(|| {
            black_box(extract_imports(black_box(&wasm)).unwrap());
        });
        let chk = bench(|| {
            black_box(check_workload(black_box(&wasm), &granted).unwrap());
        });
        let full = bench(|| {
            black_box(
                rt.block_on(svc.create_session_with_wasm("bench", Some(black_box(&wasm))))
                    .unwrap(),
            );
        });
        let size = if wasm.len() >= 1 << 20 {
            format!("{:.1} MiB", wasm.len() as f64 / (1 << 20) as f64)
        } else {
            format!("{:.1} KiB", wasm.len() as f64 / 1024.0)
        };
        println!(
            "| {label} | {size} | {imports} | {} | {} | {} |",
            fmt(ex.median),
            fmt(chk.median),
            fmt(full.median)
        );
    }
    // Denied workload: crypto-only entity loading a module that needs storage.
    let wasm = synth_module(5, 10, 0);
    let deny = bench(|| {
        black_box(
            rt.block_on(svc.create_session_with_wasm("restricted", Some(&wasm)))
                .is_err(),
        );
    });
    println!(
        "\nRejecting a non-compliant workload (small module): median {}, p95 {}.",
        fmt(deny.median),
        fmt(deny.p95)
    );

    // ------------------------------------------------------------------
    // 3. Policy loading
    // ------------------------------------------------------------------
    header("Policy loading");
    let example_path = concat!(env!("CARGO_MANIFEST_DIR"), "/policies/example.yaml");
    let example = std::fs::read_to_string(example_path)?;
    let n_entities = PolicyConfig::from_yaml(&example)?.entities.len();
    row(
        &format!("Parse + validate `policies/example.yaml` ({n_entities} entities)"),
        &bench(|| {
            black_box(PolicyConfig::from_yaml(black_box(&example)).unwrap());
        }),
    );
    let mut big = String::from("version: \"1.0\"\nentities:\n");
    for i in 0..1000 {
        big.push_str(&format!(
            "  - id: \"e{i}\"\n    capabilities:\n      crypto: true\n      random: true\n    rate_limits:\n      crypto:\n        operations_per_second: 100\n        burst_size: 200\n"
        ));
    }
    row(
        "Parse + validate a 1 000-entity policy",
        &bench(|| {
            black_box(PolicyConfig::from_yaml(black_box(&big)).unwrap());
        }),
    );

    // ------------------------------------------------------------------
    // 4. HTTP API (loopback)
    // ------------------------------------------------------------------
    let http_svc = Arc::new(EnforcementService::new(policy)?);
    let listener = rt.block_on(tokio::net::TcpListener::bind("127.0.0.1:0"))?;
    let addr = listener.local_addr()?;
    rt.spawn(async move {
        axum::serve(listener, create_router(http_svc))
            .await
            .unwrap()
    });

    let body = r#"{"entity_id":"bench"}"#;
    header("HTTP API (loopback, new TCP connection per request)");
    row(
        "`POST /api/v1/hal/access` (create session)",
        &bench(|| {
            rt.block_on(http_post(addr, "/api/v1/hal/access", body));
        }),
    );

    let conc = 32;
    let dur = Duration::from_secs(3);
    let done = Arc::new(AtomicU64::new(0));
    rt.block_on(async {
        let start = Instant::now();
        let mut tasks = Vec::new();
        for _ in 0..conc {
            let done = done.clone();
            tasks.push(tokio::spawn(async move {
                while start.elapsed() < dur {
                    http_post(addr, "/api/v1/hal/access", body).await;
                    done.fetch_add(1, Ordering::Relaxed);
                }
            }));
        }
        for t in tasks {
            t.await.unwrap();
        }
    });
    println!(
        "\nThroughput with {conc} concurrent clients: {:.0} session requests/s.",
        done.load(Ordering::Relaxed) as f64 / dur.as_secs_f64()
    );

    Ok(())
}
