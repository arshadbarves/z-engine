# Z Engine

A terminal-based AI coding agent, in the spirit of Claude Code / opencode —
built as a personal daily driver. You type a task ("fix this failing test");
Z Engine reads files, runs commands, edits code and verifies the result,
asking for approval before anything destructive.

```
you ❯ fix the failing test
zengine ⚙ read_file src/lib.rs ─ lines 1–25
zengine ✓ bash (0): cargo test 2>&1 | tail -15
zengine ✗ edit_file src/lib.rs (fuzzy)   ← approval modal shows the diff
✓ done
```

## Install

```bash
cargo install --path crates/z-engine-tui
# binary: zengine
```

Push or merge to the `release` branch (or run **Actions → release** by hand)
to build the desktop app and CLI for macOS, Windows, and Linux. Installers
are attached to the GitHub Release matching the version in
`crates/z-engine-gui/src-tauri/tauri.conf.json`.

Requirements: Rust stable (≥1.85), a Rust toolchain for projects it works
on, `ZENGINE_API_KEY` (OpenRouter by default) or a local OpenAI-compatible
server. Optional: `ripgrep` (grep falls back to pure Rust), `rust-analyzer`
(LSP tools fall back to tree-sitter/`cargo check`).

## Quick start

```bash
cd your-project
export ZENGINE_API_KEY=sk-or-...
zengine                 # TUI
zengine --headless "fix the failing test" [--auto-approve]   # one-shot
zengine --resume        # pick up a previous session
```

Keys: **Enter** send · **Esc** abort turn · **PgUp/PgDn** scroll ·
approval modal **y**(once) **s**(session prefix) **p**(persist to project)
**n/Esc/Ctrl-C**(deny) · **Ctrl-C twice** quit.
Slash commands: `/compact`, `/notes`.

## Configuration

Ladder (lowest→highest): defaults < `~/.config/z-engine/config.toml` <
`<project>/.z-engine/config.toml` < env (`ZENGINE_MODEL`, `ZENGINE_BASE_URL`)
< CLI flags.

Legacy `HARNESS_*` env vars, `~/.config/harness`, and `<project>/.harness`
are still **read** when the new names/paths are missing. New writes go to
the `z-engine` locations.

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

API key: `ZENGINE_API_KEY` env var (or `HARNESS_API_KEY`), or a single-line
file at `~/.config/z-engine/api-key` (then `~/.config/harness/api-key`).
It never lives in config.toml.

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
`~/Library/Application Support/z-engine/sessions/<ulid>.jsonl`
(macOS), `~/.local/share/z-engine/sessions` (Linux), or
`%APPDATA%\z-engine\sessions` (Windows). Crashes tear at most the last
line; `--session <ulid>` replays and continues. Existing `harness/sessions`
files are still listed.

On Windows the GUI uses a native title bar. The `bash` tool prefers Git
Bash (`bash -lc`) when it is on `PATH`, otherwise `cmd.exe /C`. Install
[Git for Windows](https://git-scm.com/download/win) for POSIX commands.

## Architecture

Two crates: `z-engine-core` (agent loop, provider, tools, permissions,
context engine, sessions, LSP/MCP — no UI deps, fully unit-tested against
recorded fixtures and a mocked SSE provider) and `z-engine-tui` (ratatui
rendering + input only). The desktop shell is `z-engine-gui`. The full
build spec and its evolution live in `docs/superpowers/specs/` (historical
`harness` names); deviations are logged in `docs/deviations.md`;
per-version acceptance evidence in `docs/ROADMAP.md`.
