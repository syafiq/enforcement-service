#!/usr/bin/env bash
# Demo driver. Builds once, then runs the two-modes example.
#   ./demo.sh          # interactive, pauses for ENTER between sections
#   ./demo.sh auto     # no pauses (dry-run / CI)
set -euo pipefail
cd "$(dirname "$0")"

echo ">>> building (one-time, silent)..."
cargo build --example two_modes_demo --quiet

clear
if [[ "${1:-}" == "auto" ]]; then
  DEMO_AUTO=1 cargo run --example two_modes_demo --quiet
else
  cargo run --example two_modes_demo --quiet
fi
