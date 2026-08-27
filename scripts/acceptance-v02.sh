#!/usr/bin/env bash
# v0.2 live acceptance: implement a multi-file feature without leaving zengine.
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

cd tmp/acceptance-v02
echo "== baseline =="
cargo test 2>&1 | grep "test result"

BIN="${BIN:-../../target/release/zengine}"; [ -x "$BIN" ] || BIN=../../target/debug/zengine

echo "== zengine --headless =="
"$BIN" \
  --project . \
  --base-url "$BASE" \
  --model "$MODEL" \
  --headless "Add priority support to this task app. Requirements: (1) in src/tasks.rs, Task gains a private priority: u8 field defaulting to 1, with getter priority(&self) -> u8 and setter set_priority(&mut self, u8); (2) display() now renders tasks as '[P1] title' using the priority; (3) src/main.rs accepts an optional second CLI arg parsed as u8 that sets the task's priority before printing; (4) add unit tests in src/tasks.rs covering default priority = 1, set_priority, and the '[P3]' display format. Run cargo test until everything is green." \
  --auto-approve

echo "== feature checks =="
cargo test 2>&1 | grep "test result"
cargo run --quiet -- "write docs" 3 | grep -q "\[P3\] write docs" && echo "CLI PRIORITY OK"
