#!/usr/bin/env bash
# v0.1 live acceptance: fix a failing test in the scratch repo, end-to-end.
# Key comes from ~/.config/z-engine/api-key or ~/.config/harness/api-key
# (never echoed, never logged).
set -euo pipefail
cd "$(dirname "$0")/.."

KEY_FILE=""
for f in "$HOME/.config/z-engine/api-key" "$HOME/.config/harness/api-key"; do
  if [ -f "$f" ]; then KEY_FILE="$f"; break; fi
done
[ -n "$KEY_FILE" ] || { echo "missing ~/.config/z-engine/api-key"; exit 2; }
export ZENGINE_API_KEY="$(cat "$KEY_FILE")"
export HARNESS_API_KEY="$ZENGINE_API_KEY"

MODEL="${ZENGINE_MODEL:-${HARNESS_MODEL:-openrouter/auto}}"
BASE="${ZENGINE_BASE_URL:-${HARNESS_BASE_URL:-https://openrouter.ai/api/v1}}"

cd tmp/acceptance-v01
echo "== before =="
cargo test 2>&1 | tail -3 || true

echo "== zengine --headless =="
../../target/debug/zengine \
  --project . \
  --base-url "$BASE" \
  --model "$MODEL" \
  --headless "The test suite in this project is failing. Run cargo test, find the bug, fix it by editing src/lib.rs only, then rerun cargo test until green. Do not modify tests." \
  --auto-approve

echo "== after =="
cargo test 2>&1 | tail -3
