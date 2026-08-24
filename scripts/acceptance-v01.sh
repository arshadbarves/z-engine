#!/usr/bin/env bash
# v0.1 live acceptance: fix a failing test in the scratch repo, end-to-end.
# Key comes from ~/.config/harness/api-key (never echoed, never logged).
set -euo pipefail
cd "$(dirname "$0")/.."

KEY_FILE="$HOME/.config/harness/api-key"
[ -f "$KEY_FILE" ] || { echo "missing $KEY_FILE"; exit 2; }
export HARNESS_API_KEY="$(cat "$KEY_FILE")"

MODEL="${HARNESS_MODEL:-openrouter/auto}"
BASE="${HARNESS_BASE_URL:-https://openrouter.ai/api/v1}"

cd tmp/acceptance-v01
echo "== before =="
cargo test 2>&1 | tail -3 || true

echo "== harness --headless =="
../../target/debug/harness \
  --project . \
  --base-url "$BASE" \
  --model "$MODEL" \
  --headless "The test suite in this project is failing. Run cargo test, find the bug, fix it by editing src/lib.rs only, then rerun cargo test until green. Do not modify tests." \
  --auto-approve

echo "== after =="
cargo test 2>&1 | tail -3
