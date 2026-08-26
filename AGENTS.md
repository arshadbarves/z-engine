# AGENTS.md — Structure Contract (read before writing code)

This repository is an AI coding-agent harness. **Any agent or human
modifying this codebase MUST maintain the structure defined below.** The
structure exists so every file stays small, single-purpose, and easy to
navigate. Violations are review-blocking.

## Golden rules

1. **File budget:** target ≤300 lines; hard cap 400. When a file would
   exceed the cap, split it by responsibility — never by percentage.
2. **One file = one reason to change** (SRP). A file named after a thing
   contains only that thing.
3. **`mod.rs` / `lib.rs` are composition roots only**: module
   declarations + re-exports. No logic beyond ~30 lines of glue.
4. **Prompts are data, not code.** All LLM prompt prose lives in
   `crates/harness-core/prompts/*.md`, loaded via `include_str!` in
   `src/prompts.rs`. Never inline prompt text inside logic files.
5. **Dependency direction (DIP):**
   ```
   harness-provider   ←  transport only (HTTP/SSE/types), no agent logic
        ↑
   harness-core       ←  brain: agent loop, tools, perms, context,
        ↑                session, config, prompts. NO UI dependencies.
        ↑
   harness-tui  /  harness-gui   ←  frontends; may import core, never
                                    the reverse
   ```
   Core must not depend on TUI/GUI; provider must not depend on core.
   Cross-layer calls go through traits/re-exported types, never
   concrete internals.
6. **Errors:** libraries (`-core`, `-provider`) use typed
   `thiserror` enums. Application shells (tui/gui) may use `anyhow`.
7. **Tests live next to what they test** (`#[cfg(test)] mod tests`) or
   in `tests/` for integration flows. One concern per integration file.

## Layout

```
crates/
├── harness-provider/          # LLM transport (swap-friendly seam)
│   ├── src/lib.rs             #   re-exports only
│   ├── src/{types,client,sse,accumulate}.rs
│   └── tests/fixtures/sse/    #   recorded SSE streams as fixtures
├── harness-core/
│   ├── prompts/               # ✏️ EDIT PROMPTS HERE (plain markdown)
│   │   ├── system-main.md     #   L0 operating instructions
│   │   ├── reviewer.md        #   post-edit reviewer persona
│   │   ├── summarizer.md      #   compaction summarizer
│   │   └── subagent.md        #   research sub-agent persona
│   └── src/
│       ├── lib.rs             # re-exports only
│       ├── prompts.rs         # include_str! registry (one const per prompt)
│       ├── agent/
│       │   ├── mod.rs         # composition root
│       │   ├── config.rs      # LoopConfig
│       │   ├── handle.rs      # AgentHandle lifecycle/spawn
│       │   ├── task.rs        # command loop, MCP/LSP wiring
│       │   ├── turn.rs        # single-turn pipeline
│       │   ├── execute.rs     # tool execution + approval gating
│       │   ├── stream.rs      # stream consumption
│       │   ├── state.rs       # LoopState
│       │   ├── revert.rs      # rewind handlers
│       │   ├── subagent.rs    # isolated research loops
│       │   ├── side_requests.rs # review + summarize calls
│       │   ├── system_prompt.rs # L0 assembly (uses crate::prompts)
│       │   └── events.rs      # Event/Command enums (UI contract)
│       ├── config/
│       │   ├── mod.rs         # composition root
│       │   ├── types.rs       # Config/FileFormat/errors
│       │   ├── loader.rs      # load + layering
│       │   └── store.rs       # persistence CRUD (atomic writes!)
│       ├── perms/
│       │   ├── mod.rs         # composition root
│       │   ├── engine.rs      # PolicyEngine decisions
│       │   └── shell_syntax.rs# tokenizer + safe-lists
│       ├── context/
│       │   ├── mod.rs         # L0 assembly + AGENTS.md loader
│       │   ├── budget.rs compact.rs cost.rs notes.rs repo_map.rs
│       ├── tools/
│       │   ├── mod.rs         # Tool trait + registry (+ re-exports)
│       │   ├── context.rs     # ToolCtx (the per-call capability bundle)
│       │   ├── fsutil.rs      # atomic_write, diffs, truncation
│       │   └── <tool_name>.rs # ONE FILE PER TOOL (bash, edit_file, …)
│       ├── lsp/  mcp/         # external-process integrations
│       └── session/           # JSONL transcript store
├── harness-tui/src/
│   ├── main.rs                # terminal setup/teardown only
│   ├── app/
│   │   ├── mod.rs state.rs input.rs reducer.rs run.rs
│   └── views/                 # PURE render fns over &App (no mutation)
└── harness-gui/src-tauri/src/
    ├── main.rs                # builder wiring only (<160 lines)
    ├── state.rs event_bridge.rs git_util.rs catalog.rs
    ├── slash_commands.rs session_store.rs
    └── commands/              # ALL #[tauri::command] fns, grouped by domain
        ├── mod.rs agent.rs settings.rs misc.rs
```

The React frontend lives in `harness-gui/ui/src`: `components/` (one
file per component), `lib/` (stores + typed `commands.ts` boundary).
Backend access goes ONLY through `lib/commands.ts`; event handling only
through `lib/events.ts`.

## How to add things (follow exactly)

| Adding… | Do this |
|---|---|
| a tool | new `tools/<name>.rs` implementing `Tool`; register in `ToolRegistry::builtins()` |
| a prompt | new `prompts/<name>.md` + one `pub const` in `src/prompts.rs` + reference it |
| an IPC command | fn in the matching `commands/<domain>.rs` with `#[tauri::command]` + add to `generate_handler!` in `main.rs`; frontend wrapper in `ui/src/lib/commands.ts` |
| a config key | `config/types.rs` (struct + Partial) → `loader.rs` apply → default in `types.rs` |
| an event variant | `agent/events.rs` enum + its serde shape in one place |

## Before you commit

```bash
cargo fmt --all
cargo clippy --workspace --all-targets   # 0 warnings required
cargo test --workspace                   # all green required
wc -l $(git diff --name-only | grep '\.rs$')   # respect the 400 cap
```

If your change pushes any file past 400 lines, split it first. If you
find an existing violation, fix the part you touch; do not grow it.
