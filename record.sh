#!/usr/bin/env bash
# Demo driver — two demos:
#
#   Demo 1: Manual mode. Hand-written YAML; one workload passes,
#           a second is refused at session creation.
#
#   Demo 2: Auto mode + tamper. LLM verdict is frozen into a YAML
#           policy. The original workload passes. A modified WASM
#           with one extra import is refused.
#
# Required env (only for Demo 2):
#   OPENAI_API_KEY   e.g. sk-...
#   LLM_ENDPOINT     e.g. https://api.deepseek.com/v1/chat/completions
#   LLM_MODEL        e.g. deepseek-chat
#
# Variants:
#   ./record.sh          interactive (pause on ENTER between shots)
#   ./record.sh auto     no pauses (dry-run / timing check)

set -euo pipefail
cd "$(dirname "$0")"

: "${LLM_ENDPOINT:=https://api.deepseek.com/v1/chat/completions}"
: "${LLM_MODEL:=deepseek-chat}"
export LLM_ENDPOINT LLM_MODEL

AUTO=${1:-}
pause() {
  [[ "$AUTO" == "auto" ]] && { echo; return; }
  printf '\n\033[2m[ENTER for %s]\033[0m ' "$1"
  read -r _
}

shot() {
  printf '\n\033[1;36m=== %s ===\033[0m\n\n' "$1"
}

# ---------------------------------------------------------------------------
# Pre-build silently so on-camera commands don't pause to compile.
# ---------------------------------------------------------------------------
echo ">>> warming caches (silent build)..."
cargo build --example run_workload                       --quiet
cargo build --example freeze_policy   --features openai  --quiet
cargo run   --example build_demo_wasms                   --quiet >/dev/null
clear

############################################################################
# DEMO 1 — Manual mode
############################################################################

shot "Demo 1 / Shot 1 — Manual policy"
if command -v bat >/dev/null 2>&1; then
  bat --paging=never --style=plain policies/demo-manual.yaml
else
  cat policies/demo-manual.yaml
fi
pause "matching workload"

shot "Demo 1 / Shot 2 — crypto-only workload against crypto-worker (PASS)"
cargo run --example run_workload --quiet -- \
  policies/demo-manual.yaml crypto-worker demo-wasms/crypto-only.wasm
pause "non-matching workload"

shot "Demo 1 / Shot 3 — sockets-app workload against crypto-worker (DENIED)"
cargo run --example run_workload --quiet -- \
  policies/demo-manual.yaml crypto-worker demo-wasms/sockets-app.wasm \
  || true
pause "Demo 2"

############################################################################
# DEMO 2 — Auto + tamper
############################################################################

if [[ -z "${OPENAI_API_KEY:-}" ]]; then
  shot "Demo 2 skipped — set OPENAI_API_KEY to run"
  exit 0
fi

shot "Demo 2 / Shot 1 — Freeze a policy from the LLM verdict"
cargo run --example freeze_policy --features openai --quiet -- \
  demo-wasms/crypto-only.wasm policies/frozen.yaml frozen-app
pause "run original workload"

shot "Demo 2 / Shot 2 — Original workload against frozen policy (PASS)"
cargo run --example run_workload --quiet -- \
  policies/frozen.yaml frozen-app demo-wasms/crypto-only.wasm
pause "run tampered workload"

shot "Demo 2 / Shot 3 — Tampered workload (extra sock_open import) — DENIED"
cargo run --example run_workload --quiet -- \
  policies/frozen.yaml frozen-app demo-wasms/crypto-only-tampered.wasm \
  || true

shot "End"
echo "Manual policies and frozen LLM verdicts behave the same way at"
echo "enforcement time. The model is consulted once, frozen, and audited."
echo
