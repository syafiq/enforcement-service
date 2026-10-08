# Performance

This page reports measured performance of the HAL Enforcement Service on AMD SEV-SNP and Intel TDX Confidential VMs, and explains how to reproduce the measurements.

## Summary

- **Checks on every HAL call:** a capability check takes **8 ns** and a rate-limit check takes **0.26–0.31 µs**, so enforcing policy adds well under a microsecond per call.
- **Admitting a workload** (reading its imports, checking them against the policy and creating a session) takes **3–3.5 µs** for a small module, **29–32 µs** for a 277 KiB module with 50 imports, and **211–241 µs** for a 4.2 MiB module with 200 imports. The time is spent mostly parsing the module, and it grows with module size.
- **Rejecting a non-compliant workload** takes about **1.2 µs**.
- **Over the HTTP API**, a session request takes **104 µs** (TDX) to **190–273 µs** (SEV-SNP) over a new loopback TCP connection. With 32 concurrent clients the service handles about **36,000** (TDX) and **23,000** (SEV-SNP) session requests per second.
- **Loading a policy** takes **67–70 µs** for the 7-entity example, and **5.0–5.4 ms** for 1,000 entities.

## Test environment

| | AMD SEV-SNP | Intel TDX |
| --- | --- | --- |
| Instance | GCP `n2d-standard-8` (8 vCPU, 32 GB), Confidential VM | GCP `c3-standard-8` (8 vCPU, 32 GB), Confidential VM |
| CPU | AMD EPYC 7B13 | Intel Sapphire Rapids |
| OS / kernel | Ubuntu 24.04.5, Linux 7.0.0-1011-gcp | Ubuntu 24.04.5, Linux 7.0.0-1011-gcp |

Toolchain: Rust 1.99.0 (`--release`). Measured on 8 October 2026 in `us-central1-a`.

## Method

The benchmark is `examples/perf.rs`.
- **Timing:** each operation is timed over 30 batches of about 20 ms, and the median and p95 per operation are reported.
- **Runs:** the benchmark ran twice per platform. Each value is the mean of the two runs' medians; where the runs differ by more than 15 %, both are shown as a range.
- **Policy:** the per-call and admission figures use an in-memory policy with a "bench" entity.
- **Test workloads:** synthetic Wasm modules built from WAT:

  | Workload | Imports | Functions | Data segment |
  | --- | ---: | ---: | ---: |
  | Small | 5 | 10 | none |
  | Medium | 50 | 1,000 | 256 KiB |
  | Large | 200 | 10,000 | 4 MiB |

  The imports are a mix of HAL and WASI functions.
- **Rejection test:** a crypto-only entity tries to load a module that also imports storage.
- **HTTP API:** the service runs on loopback. Each request opens a new TCP connection. The throughput test runs 32 concurrent clients for 3 s.

## Results

### Checks on every HAL call

| Operation | SEV-SNP median | SEV-SNP p95 | TDX median | TDX p95 |
| --- | ---: | ---: | ---: | ---: |
| Capability check (`has_capability`) | 8 ns | 8 ns | 8 ns | 8 ns |
| Rate-limit check (`check_rate_limit`) | 260 ns | 262 ns | 310 ns | 311 ns |
| Session lookup (`get_session`) | 458 ns | 459 ns | 556 ns | 556 ns |
| Audit event (`audit`) | 369 ns | 375 ns | 424 ns | 430 ns |

### Workload admission

| Workload | Size | Imports | Platform | Import extraction | Policy check (incl. extraction) | Full admission (`create_session_with_wasm`) |
| --- | ---: | ---: | --- | ---: | ---: | ---: |
| Small | 0.4 KiB | 5 | SEV-SNP | 844 ns | 1.1 µs | 3.5 µs |
| Small | 0.4 KiB | 5 | TDX | 828 ns | 1.1 µs | 3.1 µs |
| Medium | 277.0 KiB | 50 | SEV-SNP | 23.7 µs | 27.0 µs | 31.7 µs |
| Medium | 277.0 KiB | 50 | TDX | 22.8 µs | 24.9 µs | 29.3 µs |
| Large | 4.2 MiB | 200 | SEV-SNP | 217 µs | 238 µs | 241 µs |
| Large | 4.2 MiB | 200 | TDX | 200 µs | 207 µs | 211 µs |

The policy check includes import extraction. Full admission is the policy check plus creating the session.

Rejecting a non-compliant workload (small module): median 1.15 µs (SEV-SNP) and 1.16–1.20 µs (TDX).

### Policy loading

| Operation | SEV-SNP median | SEV-SNP p95 | TDX median | TDX p95 |
| --- | ---: | ---: | ---: | ---: |
| Parse + validate `policies/example.yaml` (7 entities) | 69.8 µs | 70.0 µs | 66.7 µs | 66.9 µs |
| Parse + validate a 1 000-entity policy | 5.4 ms | 5.5 ms | 5.0 ms | 5.1 ms |

### HTTP API (loopback, new TCP connection per request)

| Operation | SEV-SNP median | SEV-SNP p95 | TDX median | TDX p95 |
| --- | ---: | ---: | ---: | ---: |
| `POST /api/v1/hal/access` (create session) | 190 µs–273 µs | 277 µs | 104 µs | 106 µs |

| Throughput, 32 concurrent clients | Run 1 | Run 2 |
| --- | ---: | ---: |
| SEV-SNP | 23,364 req/s | 23,086 req/s |
| TDX | 36,193 req/s | 36,159 req/s |

The SEV-SNP request latency differed between the two runs (190 µs and 273 µs, with p95 of about 275 µs in both). Each request opens a new TCP connection, so this figure is mostly loopback TCP setup inside the Confidential VM, not the enforcement logic, which takes a few microseconds.

## Reproducing

```bash
sudo apt-get install -y build-essential   # C linker for Rust
cargo build --release --example perf
./target/release/examples/perf          # prints Markdown tables
```

The benchmark needs no TEE hardware. To recreate the test VMs on GCP:

```bash
gcloud compute instances create elastic-perf-snp --zone us-central1-a \
  --machine-type n2d-standard-8 --confidential-compute-type SEV_SNP \
  --maintenance-policy TERMINATE --image-family ubuntu-2404-lts-amd64 \
  --image-project ubuntu-os-cloud --boot-disk-size 50GB
gcloud compute instances create elastic-perf-tdx --zone us-central1-a \
  --machine-type c3-standard-8 --confidential-compute-type TDX \
  --maintenance-policy TERMINATE --image-family ubuntu-2404-lts-amd64 \
  --image-project ubuntu-os-cloud --boot-disk-size 50GB
```
