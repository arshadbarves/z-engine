# AGENTS.md — Structure Contract (read before writing code)

This repository is an AI coding-agent harness. **Any agent or human
modifying this codebase MUST maintain the structure defined below.** The
structure exists so every file stays small, single-purpose, and easy to
navigate. Violations are review-blocking.

Companion conventions: [Engineering & Coding Style Guide](docs/engineering/style-guide.md).
The [supervised harness](docs/architecture/supervised-harness.md) describes the
current bounded implementation and its limits.
The [GUI-first architecture](docs/architecture/agent-harness.md) and
[vertical-slice roadmap](docs/roadmap/agent-harness.md) describe staged future
work; they do not change the current layout below. Update this contract in the
same implementation slice as any crate or frontend migration.

The desktop GUI is the only product frontend. Do not add a terminal or
headless replacement. The agent's shell tool and private integration-test
fixtures remain supported; neither is a public command-line product.

## Golden rules

1. **File budget:** target ≤300 lines; hard cap 400. When a file would
   exceed the cap, split it by responsibility — never by percentage.
2. **One file = one reason to change** (SRP). A file named after a thing
   contains only that thing.
3. **`mod.rs` / `lib.rs` are composition roots only**: module
   declarations + re-exports. No logic beyond ~30 lines of glue.
4. **Prompts are data, not code.** All LLM prompt prose lives in
   `crates/z-engine-core/prompts/*.md`, loaded via `include_str!` in
   `src/prompts.rs`. Never inline prompt text inside logic files.
5. **Dependency direction (DIP):**
   ```
   z-engine-gui -> z-engine-core
   z-engine-core -> z-engine-provider  # model transport only
                 -> z-engine-runtime   # pure bounded supervisor contracts
                 -> z-engine-context   # provider-independent context packets
                 -> z-engine-project   # read-only project discovery
   ```
   Core must not depend on GUI. Provider, runtime, context, and project must
   not depend on core or GUI. Runtime and context perform no model,
   filesystem, or process I/O; project discovery reads bounded filesystem
   inputs but never executes suggested checks.
   Cross-layer calls go through traits/re-exported types, never
   concrete internals.
6. **Errors:** libraries (`-core`, `-provider`, `-runtime`, `-context`, `-project`) use typed
   `thiserror` enums. The GUI application shell may use `anyhow`.
7. **Tests live next to what they test** (`#[cfg(test)] mod tests`) or
   in `tests/` for integration flows. One concern per integration file.

## Layout

```
crates/
├── z-engine-provider/         # LLM transport (swap-friendly seam)
│   ├── src/lib.rs             #   re-exports only
│   ├── src/{types,client,sse,accumulate}.rs
│   └── tests/fixtures/sse/    #   recorded SSE streams as fixtures
├── z-engine-runtime/          # pure bounded task supervision contracts
│   ├── src/lib.rs             #   re-exports only
│   └── src/{supervisor,types}.rs
├── z-engine-context/          # bounded packets; no provider or filesystem I/O
│   ├── src/lib.rs             #   re-exports only
│   ├── src/{builder,packet,report,notes,error}.rs
│   └── tests/                #   budgets, evidence boundaries, supervision data
├── z-engine-project/          # bounded, read-only language-neutral discovery
│   ├── src/lib.rs             #   re-exports only
│   ├── src/{discovery,traversal,filesystem,markers,options,types,error}.rs
│   ├── src/parsers/           #   manifest-specific discovery; no check execution
│   └── tests/                #   profiles, malformed input, bounds, containment
├── z-engine-core/
│   ├── prompts/               # ✏️ EDIT PROMPTS HERE (plain markdown)
│   │   ├── system-main.md     #   L0 operating instructions
│   │   ├── reviewer.md        #   post-edit reviewer persona
│   │   ├── summarizer.md      #   compaction summarizer
│   │   ├── task-supervision.md #  bounded continuation guidance
│   │   ├── context-packet.md  #   task data provenance and retention
│   │   ├── subagent.md        #   research sub-agent persona
│   │   └── session-title.md   #   sidebar session title
│   └── src/
│       ├── lib.rs             # re-exports only
│       ├── prompts.rs         # include_str! registry (one const per prompt)
│       ├── agent/
│       │   ├── mod.rs         # composition root
│       │   ├── config.rs      # LoopConfig
│       │   ├── handle.rs      # AgentHandle lifecycle/spawn
│       │   ├── task.rs        # command loop, MCP/LSP wiring
│       │   ├── turn.rs        # single-turn pipeline
│       │   ├── request.rs     # grounded request assembly
│       │   ├── supervision.rs # bounded response-boundary adapter
│       │   ├── task_completion.rs # durable completion gate
│       │   ├── execute.rs     # tool execution + approval gating
│       │   ├── stream.rs      # stream consumption
│       │   ├── state.rs       # LoopState
│       │   ├── revert.rs      # rewind handlers
│       │   ├── subagent.rs    # isolated research loops
│       │   ├── auxiliary.rs   # bounded, cancellable text collection
│       │   ├── side_requests.rs # summary + title requests
│       │   ├── review.rs      # advisory reviewer outcomes
│       │   ├── system_prompt.rs # L0 assembly (uses crate::prompts)
│       │   └── events.rs      # Event/Command enums (UI contract)
│       ├── config/
│       │   ├── mod.rs         # composition root
│       │   ├── types.rs       # Config/FileFormat/errors
│       │   ├── loader.rs      # load + layering
│       │   ├── store.rs       # persistence CRUD (atomic writes!)
│       │   ├── auth.rs        # OpenRouter key in auth.json
│       │   └── paths.rs       # z-engine dirs; create config if missing
│       ├── perms/
│       │   ├── mod.rs         # composition root
│       │   ├── engine.rs      # PolicyEngine decisions
│       │   └── shell_syntax.rs# tokenizer + safe-lists
│       ├── context/
│       │   ├── mod.rs         # composition root
│       │   ├── system.rs      # L0 assembly + AGENTS.md loader
│       │   ├── task_packet.rs # observed report + unverified notes projection
│       │   ├── budget.rs compact.rs cost.rs notes.rs repo_map.rs
│       ├── tools/
│       │   ├── mod.rs         # composition root + re-exports
│       │   ├── interface.rs   # Tool contract + outcomes
│       │   ├── registry.rs    # ToolRegistry + built-in registrations
│       │   ├── context.rs     # ToolCtx (the per-call capability bundle)
│       │   ├── fsutil.rs      # atomic_write, diffs, truncation
│       │   └── <tool_name>.rs # ONE FILE PER TOOL (bash, edit_file, …)
│       ├── lsp/  mcp/         # external-process integrations
│       ├── verification/     # typed checks, artifacts, freshness and final gate
│       ├── verification/     # typed checks, evidence, freshness, completion gate
│       └── session/           # JSONL transcript store
└── z-engine-gui/src-tauri/src/
    ├── main.rs                # builder wiring only (<160 lines)
    ├── state.rs event_bridge.rs git_util.rs catalog.rs
    ├── slash_commands.rs session_store.rs
    └── commands/              # ALL #[tauri::command] fns, grouped by domain
        ├── mod.rs agent.rs settings.rs misc.rs
```

The desktop frontend is **Svelte 5 + Bits UI + Vite** in
`z-engine-gui/ui/src`. Canonical UI rules:
[`docs/design/gui-ui-guide.md`](docs/design/gui-ui-guide.md).

```
ui/src/
├── App.svelte              # composition root (wiring only)
├── lib/commands.ts         # ONLY Tauri invoke wrappers
├── lib/runtime/            # agent events, transcript, session park/replay
├── lib/domain/             # pure helpers (tested with vitest)
├── lib/stores/             # config / workspace / update / chrome UI
├── lib/ui/                 # Bits UI wrappers + Icon + Button (ONLY bits-ui import)
└── components/{chrome,sidebar,chat,settings,overlays}/
```

Rules: screens never `invoke()` or import `bits-ui`; event listening only
in `lib/runtime/listen.ts`; file budget ≤300 / hard cap 400. Do not add
SvelteKit, Tailwind, shadcn-svelte, React, or a second design system.

## How to add things (follow exactly)

| Adding… | Do this |
|---|---|
| a tool | new `tools/<name>.rs` implementing `Tool`; register in `ToolRegistry::builtins()` |
| a prompt | new `prompts/<name>.md` + one `pub const` in `src/prompts.rs` + reference it |
| an IPC command | fn in the matching `commands/<domain>.rs` with `#[tauri::command]` + add to `generate_handler!` in `main.rs`; frontend wrapper in `ui/src/lib/commands.ts` |
| a GUI screen | new `ui/src/components/<area>/<Name>.svelte`; use `lib/ui` primitives; follow `docs/design/gui-ui-guide.md` |
| a GUI primitive | wrapper in `ui/src/lib/ui/` around Bits UI; never import `bits-ui` from a screen |
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
