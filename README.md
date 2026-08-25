# harness

A terminal-based AI coding agent, in the spirit of Claude Code / opencode —
built as a personal daily driver. You type a task ("fix this failing test");
harness reads files, runs commands, edits code and verifies the result,
asking for approval before anything destructive.

```
you ❯ fix the failing test
harness ⚙ read_file src/lib.rs ─ lines 1–25
harness ✓ bash (0): cargo test 2>&1 | tail -15
harness ✗ edit_file src/lib.rs (fuzzy)   ← approval modal shows the diff
✓ done
```

## Install

```bash
cargo install --path crates/harness-tui
# binary: harness
```

Requirements: Rust stable (≥1.85), a Rust toolchain for projects it works
on, `HARNESS_API_KEY` (OpenRouter by default) or a local OpenAI-compatible
server. Optional: `ripgrep` (grep falls back to pure Rust), `rust-analyzer`
(LSP tools fall back to tree-sitter/`cargo check`).

## Quick start

```bash
cd your-project
export HARNESS_API_KEY=sk-or-...
harness                 # TUI
harness --headless "fix the failing test" [--auto-approve]   # one-shot
harness --resume        # pick up a previous session
```

Keys: **Enter** send · **Esc** abort turn · **PgUp/PgDn** scroll ·
approval modal **y**(once) **s**(session prefix) **p**(persist to project)
**n/Esc/Ctrl-C**(deny) · **Ctrl-C twice** quit.
Slash commands: `/compact`, `/notes`.

## Configuration

Ladder (lowest→highest): defaults < `~/.config/harness/config.toml` <
`<project>/.harness/config.toml` < env (`HARNESS_MODEL`, `HARNESS_BASE_URL`)
< CLI flags.

```toml
model = "anthropic/claude-sonnet-4"
base_url = "https://openrouter.ai/api/v1"      # or http://localhost:11434/v1
max_context_tokens = 120000
review = true                                   # post-edit reviewer pass

[permissions]
allow = ["cargo test*", "git status"]           # bash prefix rules

[cost]                                          # optional calibration
# usd_per_mtok_input / usd_per_mtok_output are read from the built-in table;
# per-model overrides land with your provider config.

[mcp.servers.echo]
command = "python3"
args = ["scripts/mcp_echo_server.py"]
```

The API key never enters config files: `HARNESS_API_KEY` only.

## Tools the model gets

`bash` (persistent cwd, env allowlist, timeout+kill of the process group,
output truncation to spill files) · `read_file` · `write_file` /
`edit_file` (exact → line-hint → fuzzy ladder, read-before-edit enforced) ·
`glob` · `grep` (ripgrep fast-path) · `update_context_notes` · `task`
(isolated read-only sub-agent) · `go_to_definition` / `find_references` /
`lsp_diagnostics` (rust-analyzer; falls back to outlines + `cargo check`)
plus any MCP externals you register.

## How it stays on budget

Provider-reported usage drives a meter (warn ≥80%, auto-compaction ≥92%):
old tool outputs elide to spill files, old prose is summarized into durable
context notes via a side-request, and `/compact` forces it on demand.

## Sessions

Every turn appends newline-delimited JSON events under
`~/Library/Application Support/harness/sessions/<ulid>.jsonl`
(`~/.local/share/harness/sessions` on Linux). Crashes tear at most the last
line; `--session <ulid>` replays and continues.

## Architecture

Two crates: `harness-core` (agent loop, provider, tools, permissions,
context engine, sessions, LSP/MCP — no UI deps, fully unit-tested against
recorded fixtures and a mocked SSE provider) and `harness-tui` (ratatui
rendering + input only). The full build spec and its evolution live in
`docs/superpowers/specs/`; deviations are logged in `docs/deviations.md`;
per-version acceptance evidence in `docs/ROADMAP.md`.
