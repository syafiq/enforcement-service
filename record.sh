#!/usr/bin/env bash
# Demo driver for screen recording. Pre-builds everything, then walks the
# four on-camera "shots" with ENTER pauses between them.
#
# Required env (DeepSeek shown; any OpenAI-compatible endpoint works):
#   OPENAI_API_KEY   e.g. sk-...
#   LLM_ENDPOINT     e.g. https://api.deepseek.com/v1/chat/completions
#   LLM_MODEL        e.g. deepseek-chat
#
# Variants:
#   ./record.sh          interactive (pause on ENTER between shots)
#   ./record.sh auto     no pauses (dry-run / timing check)

set -euo pipefail
cd "$(dirname "$0")"

if [[ -z "${OPENAI_API_KEY:-}" ]]; then
  echo "ERROR: OPENAI_API_KEY not set" >&2
  exit 1
fi
: "${LLM_ENDPOINT:=https://api.deepseek.com/v1/chat/completions}"
: "${LLM_MODEL:=deepseek-chat}"
export OPENAI_API_KEY LLM_ENDPOINT LLM_MODEL

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
# Pre-build everything silently so on-camera commands don't pause to compile.
# ---------------------------------------------------------------------------
echo ">>> warming caches (silent build)..."
cargo build --example two_modes_demo                         --quiet
cargo build --example openai_analyzer  --features openai     --quiet
cargo run   --example build_demo_wasms                       --quiet >/dev/null
clear

# ---------------------------------------------------------------------------
# Shot 1 — the policy file
# ---------------------------------------------------------------------------
shot "Shot 1 — Policy file (manual entries + one auto entry)"
if command -v bat >/dev/null 2>&1; then
  bat --paging=never --style=plain policies/example.yaml | tail -50
else
  tail -50 policies/example.yaml
fi
pause "manual mode + auto refusal"

# ---------------------------------------------------------------------------
# Shot 2 — manual mode works, auto without WASM refuses
# (re-uses the existing two_modes_demo, but only sections 1-3)
# ---------------------------------------------------------------------------
shot "Shot 2 — Manual mode grants YAML caps; auto refuses without a workload"
DEMO_AUTO=1 cargo run --example two_modes_demo --quiet 2>&1 \
  | sed -n '/=== 2\./,/=== 4\./p' \
  | sed '/=== 4\./d'
pause "real LLM verdicts (3 contrasting workloads)"

# ---------------------------------------------------------------------------
# Shot 3 — three real round-trips to the LLM, three different verdicts
# ---------------------------------------------------------------------------
for w in sockets-app crypto-only pure-compute; do
  shot "Shot 3 — $w.wasm   (live $LLM_MODEL call)"
  cargo run --example openai_analyzer --features openai --quiet \
    -- "demo-wasms/${w}.wasm"
  pause "next workload"
done

# ---------------------------------------------------------------------------
# Shot 4 — wrap-up
# ---------------------------------------------------------------------------
shot "Shot 4 — Audit"
cat <<'EOF'
Every session records its policy_source:

  Manual entity   -> PolicySource::Manual
  Auto   entity   -> PolicySource::Auto { model: "deepseek-chat" }

Two modes. No intersection. Pick the one that fits the workload.
EOF
echo
