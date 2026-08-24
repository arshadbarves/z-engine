# AGENT BRIEF: Build "harness" — a personal Rust CLI coding agent

You are an autonomous coding agent tasked with building **harness**: a terminal-based AI
coding assistant written in Rust, similar in spirit to Claude Code / opencode, for daily
personal use. This document is a complete, self-contained specification. Work through it
version by version. Do not skip ahead; each version must pass its acceptance test before
the next begins.

---

## 1. MISSION

Build a TUI coding agent where the user types a task ("fix this failing test") and the
agent autonomously reads files, runs commands, edits code, and verifies results — with
approval gates before any mutating action.

**Primary users:** the repo owner, daily, on real projects.
**Definition of success for v1.0:** the owner uses it as their daily driver instead of
commercial tools.

## 2. HARD CONSTRAINTS

- Language: **Rust** (stable). Async runtime: **tokio**.
- UI: **ratatui + crossterm** full TUI.
- LLM access: **OpenAI-compatible Chat Completions API only** (works with OpenRouter,
  Ollama, LM Studio, Groq, etc.). One provider adapter. No native Anthropic/Google
  adapters in v1.
- Streaming via **SSE** (`stream: true`), including streamed tool-call deltas.
- **No vector database, no embeddings.** Context selection = tree-sitter AST + grep +
  explicit model annotations.
- No external process dependencies at runtime except optionally `ripgrep` (fall back to
  pure-Rust search if absent) and LSP servers (v0.8+).
- Secrets via environment variables only (`HARNESS_API_KEY`); never logged, never
  committed.

## 3. WORKSPACE STRUCTURE

Cargo workspace, two crates, compiler-enforced boundary between brain and face:

```
crates/
  harness-core/        # lib: NO UI DEPENDENCIES, fully unit-testable
    src/
      agent/           # loop orchestration, turn state machine, cancellation
      provider/        # OpenAI-compatible HTTP client, SSE parsing, tool-call schema
      tools/           # Tool trait + built-ins (bash, read, write, edit, glob, grep...)
      context/         # system-prompt builder, AGENTS.md loader, token counter,
                       # compaction engine, context-notes store
      perms/           # policy engine: allowlists, approval decision enum
      session/         # JSONL transcript append/read/replay
      config/          # global + project config.toml loading, layering
  harness-tui/         # bin: rendering + input ONLY; all logic lives in core
    src/
      app.rs           # owns AgentHandle, consumes events, redraws
      views/           # chat view, diff view, approval modal, session picker
      statusbar.rs     # model · tokens · est. cost · session id
```

Communication core→TUI: async mpsc channel of typed events (`TokenDelta`, `ToolCallStarted`,
`ApprovalRequired{request_id}`, `TurnCompleted`, `Error`, ...). TUI→core: command channel
(`SubmitMessage`, `Approve{id}`, `Deny{id}`, `Abort`). The TUI never calls tools or the
provider directly.

## 4. CORE ABSTRACTIONS

### 4.1 Tool trait — the only extension seam

```rust
#[async_trait::async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters_schema(&self) -> serde_json::Value; // JSON Schema
    async fn run(&self, input: serde_json::Value, ctx: &ToolCtx) -> Result<ToolOutput>;
}
```

`ToolCtx` carries cwd, permission checker handle, token budget meter, abort flag.
MCP tools (v0.9) implement the same trait — the loop never knows the difference.

### 4.2 Agent loop

```text
user msg → [context builder] → POST /chat/completions (stream)
   ▲                                   │
   │                     ┌─ text tokens → TUI (streamed)
   │                     └─ tool_calls → perms.decide(call):
   │                            Allow → execute → result pushed as tool message → loop
   │                            Gate  → ApprovalRequired event → wait on cmd channel
   │                                      Approve → execute → loop ; Deny → refusal msg → loop
   └── repeat while response contains tool calls. No hard turn cap;
       Esc/Ctrl-C aborts via Abort command (instant, mid-stream).
```

Loop rules:
- Parallel tool calls in one response are executed concurrently unless flagged unsafe.
- Every tool result is truncated for context (head+tail) before entering the transcript;
  full output goes to a temp file whose path is included in the truncated result.
- Errors from tools are returned TO THE MODEL as tool-result messages (self-correction),
  never crash the loop. Only unrecoverable transport errors end the turn.

## 5. PERMISSIONS MODEL

- Reads (`read_file`, `glob`, `grep`) and safe info commands: auto-allowed.
- Mutating tools (`bash`, `write_file`, `edit_file`): gated by approval modal with four
  answers: `yes once` / `always this prefix (session)` / `always (persist allowlist)` /
  `no`.
- Allowlist entries are command-prefix rules (e.g. `cargo test*`, `git status`),
  persisted in `<project>/.harness/config.toml`, layered over
  `~/.config/harness/config.toml`.
- Writes outside the project root always prompt; "always persist" disabled for them.
- Denial sends a polite refusal message into the conversation so the model reroutes.

## 6. CONTEXT ENGINE (the differentiator)

No dumb caps. Token-budget-driven progressive demotion. Priority ladder, compacted
bottom-up under pressure:

```text
L0  system prompt + AGENTS.md            never modified (stable prefix ⇒ provider cache hits)
L1  context notes                        survives all compaction verbatim
L2  recent N turns                       kept verbatim
L3  old turns                            summarized into L1 notes when demoted
L4  old tool outputs                     replaced by "[elided; full: /tmp/harness/out-42.log]"
```

### 6.1 Context-notes protocol (meta-output)

Model self-reports via a pseudo-tool each turn (batchable alongside real tool calls):

```json
{ "progress": "rewrote lexer fn, tests still red",
  "decisions": ["no regex — hand parser"],
  "droppable": ["200-line grep output above"],
  "needs_later": ["error enum shape for main.rs wiring"] }
```

Compactor treats notes as ground truth: `droppable` demotes immediately even without
pressure; `decisions` promote to L1. Implicit signal: read/edit access counters per file
combine with notes to rank what the repo-map refreshes.

Pressure measurement: cumulative usage deltas from provider responses vs configured
budget (default ~120k). At ≥80% the status bar warns; compaction runs automatically at
≥92% using a structured-summarization prompt (facts / decisions / open threads), keeping
L2 verbatim. Never kill the task over context.

## 7. TOOL SPECIFICATIONS

| Tool | Behavior | Ships |
|---|---|---|
| `bash` | persistent shell cwd; env allowlist; timeout 60s default; head+tail truncation w/ temp-file path | v0.1 |
| `read_file` | line-numbered; `offset`/`limit`; binary detection | v0.1 |
| `write_file` | full-file write; returns unified diff in result | v0.2 |
| `edit_file` | old_string/new_string; match ladder: exact → line-range hint → fuzzy(Levenshtein≤threshold); refuses if file not read this session (read-before-edit enforced via file-state tracker) | v0.2 |
| `glob` | pattern → paths, capped | v0.2 |
| `grep` | regex over files (ripgrep-style flags), capped matches w/ line numbers | v0.2 |
| `update_context_notes` | §6.1 schema | v0.3 |
| `task` | spawns isolated sub-loop (own transcript, read-only toolset default); final summary string returns to parent | v0.7 |
| `go_to_definition`, `find_references` | via LSP | v0.8 |
| MCP externals | stdio transport, discovered at startup from config | v0.9 |

## 8. SESSIONS & CONFIG

- Transcript: append-only JSONL, one event per line, at
  `~/.local/share/harness/sessions/<ulid>.jsonl`. Event kinds: `UserMsg`, `AssistantMsg`,
  `ToolCall`, `ToolResult`, `Note`, `Meta`. Resume = replay events into fresh loop state.
  Crash-safe by construction (never rewrite).
- Config layering: defaults < `~/.config/harness/config.toml` <
  `<project>/.harness/config.toml` < CLI flags. Keys: `model`, `base_url`,
  `max_context_tokens`, `permissions.allow[]`, `mcp.servers{}`.
- API key: `HARNESS_API_KEY` env var.

## 9. VERSIONED ROADMAP — implement strictly in order

Each version ends with: all tests green, clippy clean, feature demoed manually,
git tag `vX.Y`.

**v0.1 — Walking skeleton**
Workspace scaffold; provider client (SSE + streaming tool-call deltas); agent loop;
minimal TUI (chat view, streaming text, input box, bash approval modal);
tools `bash` + `read_file`.
✅ Acceptance: interactively fix a failing test end-to-end in a scratch repo.

**v0.2 — Editing**
`write_file`, `edit_file` (match ladder + read-before-edit), `glob`, `grep`;
diff preview inside approval modal; syntax-highlighted diffs.
✅ Acceptance: implement a multi-file feature without leaving the harness.

**v0.3 — Context engine**
AGENTS.md loader; token counter + status-bar meter; `/compact` command; notes protocol;
demotion ladder + auto-compaction.
✅ Acceptance: session stays coherent past 100k tokens (synthetic long-session test).

**v0.4 — Sessions**
JSONL persistence, resume command, session picker overlay.
✅ Acceptance: `kill -9` mid-task, restart, resume, continue correctly.

**v0.5 — Permissions hardening**
Persistent allowlists, config layering, outside-root write guard.
✅ Acceptance: repeated safe commands never re-prompt after persisting once.

**v0.6 — Repo map (tree-sitter)**
Symbol outlines per file (fns/structs/imports), reference-ranked against working set,
compact map injected after L0, refreshed on edits.
✅ Acceptance: navigate a large unfamiliar repo answering "where is X defined/used"
without grep spam.

**v0.7 — Subagents**
`task` tool spawning isolated explore/research loops.
✅ Acceptance: broad exploration leaves parent context small (measure token delta).

**v0.8 — LSP integration**
rust-analyzer client (spawn, initialize handshake, restart-on-crash);
diagnostics-after-edit hook feeds errors back into the loop automatically;
`go_to_definition` / `find_references` tools.
✅ Acceptance: deliberately broken edit is reported and fixed without manual cargo check.

**v0.9 — Review pass + MCP**
Post-edit-batch reviewer side-prompt (diff + diagnostics vs original intent) posting
findings as user-role messages; MCP stdio client registering external tools.
✅ Acceptance: seeded bug caught by review pass; one third-party MCP server usable.

**v1.0 — Polish & distribution**
Error-handling audit, cost meter calibration, `--headless` one-shot mode reading task
from argv/stdin, README, `cargo install --path crates/harness-tui`.
✅ Acceptance: used as daily driver for one week of real work.

## 10. ENGINEERING CONVENTIONS FOR THE BUILDING AGENT

- TDD where practical: unit-test provider SSE parsing against recorded fixtures, tool
  match ladders, perms matching, JSONL replay, compaction ordering. Integration-test the
  agent loop against a mocked provider (serve canned SSE).
- One version per branch/PR; conventional commits; update this spec's checkboxes as you
  go.
- Never log API keys or file contents at INFO+.
- Handle these explicitly: stream disconnects (retry with backoff, resume-safe),
  malformed tool-call JSON (return error to model), Ctrl-C during approval (deny),
  provider rate limits (backoff, surface remaining budget).
- If a requirement here proves impractical, stop, record the deviation and rationale in
  `docs/deviations.md`, choose the smallest alternative consistent with §2 constraints,
  and continue. Do not silently redesign architecture.
