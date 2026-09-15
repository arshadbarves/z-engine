# Current-state assessment (historical baseline)

Baseline: commit `31ad509`, inspected 2026-09-12.
Scope: source/configuration inspection for the GUI-first architecture proposal,
not a security audit or a fresh execution of the test suite.

The repository already has a functional coding-agent foundation. The central
gap is not a lack of tools: **the lifetime of an assistant turn is currently
used where the proposed product needs an independently assessed, durable task.**

See the [target architecture](agent-harness.md),
[runtime contracts](task-runtime.md), and
[roadmap](../roadmap/agent-harness.md) for proposed behavior. None of those
proposals should be read as current guarantees.

This assessment intentionally preserves the pre-S1 baseline, including retired
terminal paths. For current implementation, see the
[S1 completion contract](verification-completion.md) and the
[GUI-only supervised harness](supervised-harness.md).

## 1. Workspace and product surfaces

At this baseline, the workspace declared four members:

| Crate | Observed responsibility |
|---|---|
| [provider](../../crates/z-engine-provider/src/lib.rs) | OpenAI-compatible transport, SSE decoding and accumulation, wire types |
| [core](../../crates/z-engine-core/src/lib.rs) | Agent, tools, policy, configuration, context, session, LSP and MCP |
| [GUI](../../crates/z-engine-gui/src-tauri/src/main.rs) | Tauri application, commands and event bridge |
| TUI (since removed) | Terminal UI, argument handling, session selection and headless mode |

Core does not import GUI/TUI; provider is already separated. This is a useful
starting boundary, not a reason to keep expanding core indefinitely.

At the baseline, releases built desktop and CLI artifacts. Headless execution,
two shell acceptance scripts, and a Python kill/restart scenario depended on
the terminal binary. Those product surfaces have since been removed. Their
useful coverage now lives in private
[multi-file](../../crates/z-engine-core/tests/desktop_retirement_multifile.rs)
and [crash/replay](../../crates/z-engine-core/tests/desktop_retirement_crash_recovery.rs)
fixtures; the [current workflow](../../.github/workflows/release.yml) packages
only the desktop app.

The [GUI dev entrypoint](../../scripts/dev-gui.sh) and
[GUI packager](../../scripts/package-gui.sh) already exist. No additional
frontend framework or application shell is needed.

## 2. Agent lifecycle and completion

| Observation | Source | Architectural consequence |
|---|---|---|
| A background task dispatches commands and runs a turn loop | [task.rs](../../crates/z-engine-core/src/agent/task.rs), [handle.rs](../../crates/z-engine-core/src/agent/handle.rs) | Reuse ownership/channels; introduce task identity above individual turns |
| Request, stream, tools, compaction and optional review form the existing pipeline | [turn.rs](../../crates/z-engine-core/src/agent/turn.rs) | Evolve this path rather than build a parallel executor |
| No completed tool calls ends the turn as Completed | [turn.rs](../../crates/z-engine-core/src/agent/turn.rs) | A finished reply does not prove requirements or checks passed |
| Completed and aborted turns emit different events | [task.rs](../../crates/z-engine-core/src/agent/task.rs) | Keep turn notifications, but separate durable task outcome |
| Gated calls are decided before execution; concurrency-safe work may run in parallel | [execute.rs](../../crates/z-engine-core/src/agent/execute.rs) | Existing central gate is the place to add effect/evidence contracts |
| Abort is cooperative; there is no distinct durable Pause protocol | [handle.rs](../../crates/z-engine-core/src/agent/handle.rs), [execute.rs](../../crates/z-engine-core/src/agent/execute.rs) | Stop, safe suspension and crash recovery need separate states |

The current loop is more than a single completion request, but it has no
evidence-based completion assessment that combines original requirements,
repository state, changes, and check results. An empty tool-call list remains a
turn boundary, not the target product's completion gate.

## 3. Verification, roles, and prompts

- [Post-edit review](../../crates/z-engine-core/src/agent/side_requests.rs) is
  optional and advisory. Findings are injected into subsequent model context;
  a clean/no-result reviewer response is not deterministic verification.
- [Diagnostics tooling](../../crates/z-engine-core/src/tools/lsp_tools/diagnostics.rs)
  can use Rust LSP and Cargo diagnostics. Automatic attachment is gated by an
  available LSP context and Rust edits; it is not an always-on project verifier.
- [Cargo diagnostics](../../crates/z-engine-core/src/lsp/cargo_check.rs) collect
  parsed compiler messages, not a persisted check record with exit status,
  coverage and source fingerprint. Do not equate this API with a build-pass gate.
- [Isolated subagents](../../crates/z-engine-core/src/agent/subagent.rs) and
  the [read-only registry](../../crates/z-engine-core/src/tools/mod.rs) already
  support bounded research. They do not implement arbitrary role scheduling.
- [Prompt registry](../../crates/z-engine-core/src/prompts.rs) contains main,
  reviewer, summarizer, subagent and session-title prompts.
  [Main instructions](../../crates/z-engine-core/prompts/system-main.md) already
  ask for end-to-end work and verification, but remain brief and terminal-framed.
- [Prompt inspection](../../crates/z-engine-core/src/agent/prompt_inspect.rs)
  exposes assembled context. Reuse it for inspectable enhancement/strategy
  provenance rather than duplicating prompt assembly in the GUI.

There are no built-in structured question or enhancement tools/settings in the
inspected [registry](../../crates/z-engine-core/src/tools/mod.rs) and
[config model](../../crates/z-engine-core/src/config/types.rs). Specialist
planner/debugger/verifier/experience-extractor roles are proposed additions,
not existing independent agents.

## 4. Tools and repository safety

| Existing capability | Evidence | Limit of the current contract |
|---|---|---|
| Tool trait, schemas, preview and concurrency flag | [tools/mod.rs](../../crates/z-engine-core/src/tools/mod.rs) | Output is primarily text/summary plus `ok`; no general typed effects/evidence protocol |
| Read-before-edit and stale-read tracking | [ToolCtx](../../crates/z-engine-core/src/tools/context.rs), [file_state.rs](../../crates/z-engine-core/src/tools/file_state.rs) | Useful local safeguards, not a workspace transaction/recovery protocol |
| Edit ladder, full-file writes and previews | [edit_file.rs](../../crates/z-engine-core/src/tools/edit_file.rs), [write_file.rs](../../crates/z-engine-core/src/tools/write_file.rs) | Text operations, not semantic refactoring |
| Temporary-sibling file replacement | [fsutil.rs](../../crates/z-engine-core/src/tools/fsutil.rs) | Single-file replacement, not multi-file atomicity or a complete durability contract |
| Rust definition/reference/diagnostics tools | [lsp_tools](../../crates/z-engine-core/src/tools/lsp_tools), [LSP client](../../crates/z-engine-core/src/lsp/mod.rs) | Rust-focused; no built-in semantic rename/change-signature/move/extract pipeline |
| Policy decisions and command rules | [engine.rs](../../crates/z-engine-core/src/perms/engine.rs) | Extend the shared gate rather than infer authority from dynamic strategy selection |
| MCP stdio discovery/calls adapted into Tool | [MCP client](../../crates/z-engine-core/src/mcp/mod.rs), [adapter](../../crates/z-engine-core/src/mcp/tool_adapter.rs) | Shared execution seam exists; typed effects, durable reconciliation and evidence still need work |

MCP calls are serialized by the adapter, rather than advertised as
concurrency-safe. That is a reusable conservative default, not a guarantee
that remote side effects can be cancelled or rolled back.

## 5. Persistence, resume, and rewind

[Session storage](../../crates/z-engine-core/src/session/mod.rs) appends flushed
JSONL events and rebuilds working messages/notes for resume.
[Agent resume](../../crates/z-engine-core/src/agent/handle.rs) seeds the loop
from that reconstructed state. These are real transcript-recovery capabilities,
not a durable suspended operation.

[Checkpoints](../../crates/z-engine-core/src/tools/checkpoint.rs) hold managed
file preimages in memory: at most 50 retained turns, skipping files larger than
8 MiB and files changed through bash. The
[revert handlers](../../crates/z-engine-core/src/agent/revert.rs) restore tracked
files and trim transcript history, and explicitly account for checkpoint loss
after application restart.

Consequences:

- Opening old messages is not resuming a persisted execution continuation.
- File rewind has bounded, partial coverage and does not survive restart.
- There is no persisted operation-intent/outcome reconciliation protocol for
  the gap between an effect occurring and its result being stored.
- Current JSONL reuse is reasonable; stronger schema, durability and recovery
  semantics do not inherently require a new database first.

## 6. GUI and configuration integration

[GUI state](../../crates/z-engine-gui/src-tauri/src/state.rs) holds session-keyed
agent loops. The [event bridge](../../crates/z-engine-gui/src-tauri/src/event_bridge.rs)
tags events with session identity. [Agent commands](../../crates/z-engine-gui/src-tauri/src/commands/agent.rs)
cover submit, abort, approvals, mode/model control, compaction, session start and
revert. These are the backend seams for lifecycle and evidence additions.

The frontend already implements the following surfaces:

| Surface | Observed behavior / evidence |
|---|---|
| Stop | [Composer](../../crates/z-engine-gui/ui/src/components/chat/Composer.svelte) aborts an active turn; there is no separate durable Pause control |
| Turn status | [dispatch.ts](../../crates/z-engine-gui/ui/src/lib/runtime/dispatch.ts) clears busy state and renders a `done` status on `turnCompleted`; this is not evidence-gated task completion |
| User-facing modes | [ModePicker](../../crates/z-engine-gui/ui/src/components/chat/ModePicker.svelte) exposes normal, accept-edits and plan; migration must preserve their authority constraints separately from strategy |
| Approvals | [ApprovalCard](../../crates/z-engine-gui/ui/src/components/chat/ApprovalCard.svelte) supports preview, once/session/persist choices and denial |
| Background sessions | [session.ts](../../crates/z-engine-gui/ui/src/lib/runtime/session.ts) parks/restores UI snapshots and routes background-session work; parking is not process-restart suspension |
| Changes | [DiffPanel](../../crates/z-engine-gui/ui/src/components/overlays/DiffPanel.svelte) distinguishes chat-scoped changes from Git working-tree changes |
| Prompt visibility | [PromptInspector](../../crates/z-engine-gui/ui/src/components/overlays/PromptInspector.svelte) shows assembled prompt/context information, not prompt enhancement |
| IPC/event boundary | [commands.ts](../../crates/z-engine-gui/ui/src/lib/commands.ts) wraps invokes; [listen.ts](../../crates/z-engine-gui/ui/src/lib/runtime/listen.ts) owns subscriptions |

Lifecycle changes must update live event reduction, background-session routing,
and [replay](../../crates/z-engine-gui/ui/src/lib/runtime/replay.ts) together.
Replacing only the visible status message would leave the underlying completion
semantics unchanged.

[Settings commands](../../crates/z-engine-gui/src-tauri/src/commands/settings.rs)
already expose model, context, review, credentials, MCP and permission settings.
[Config types](../../crates/z-engine-core/src/config/types.rs),
[layering](../../crates/z-engine-core/src/config/loader.rs), and
[store](../../crates/z-engine-core/src/config/store.rs) are separate; new options
must be wired across all three and the GUI, not appended only to a prompt.

The [canonical GUI guide](../design/gui-ui-guide.md) requires Svelte 5, Bits UI
and Vite, typed command wrappers, centralized event listening, pure domain
helpers, and existing UI primitives. The new architecture preserves this stack.

## 7. Experience and maintainability

[Context notes](../../crates/z-engine-core/src/context/notes.rs) preserve
progress, decisions, deferred work and summaries across compaction.
These are session context, not a relevance-ranked store of verified engineering
experience or a consent-based preference-learning system.

The [structure contract](../../AGENTS.md) already mandates small files, prompt
data, dependency direction and typed library errors. Current composition files
still contain implementation logic, for example
[tools/mod.rs](../../crates/z-engine-core/src/tools/mod.rs) (204 lines),
[lsp/mod.rs](../../crates/z-engine-core/src/lsp/mod.rs) (372), and
[mcp/mod.rs](../../crates/z-engine-core/src/mcp/mod.rs) (302) at the baseline.
Extract responsibilities when touching them; do not expand these roots or
perform unrelated mass cleanup.

Existing tests include a
[mocked agent integration suite](../../crates/z-engine-core/tests/agent_loop_mocked.rs),
[real-LSP integration entrypoint](../../crates/z-engine-core/tests/lsp_real.rs),
and unit tests adjacent to tools, policy and provider logic. Their existence is
not a claim that they were run or passed during this documentation task.

The [frontend package](../../crates/z-engine-gui/ui/package.json) defines
`test` (Vitest), `check` (svelte-check), `lint` (oxlint), and `build` (Vite).
Existing [event tests](../../crates/z-engine-gui/ui/src/lib/events.test.ts),
[session runtime tests](../../crates/z-engine-gui/ui/src/lib/sessionRuntime.test.ts),
[session swap tests](../../crates/z-engine-gui/ui/src/lib/sessionSwap.test.ts), and
[approval preview tests](../../crates/z-engine-gui/ui/src/lib/approvalPreview.test.ts)
provide concrete places to extend lifecycle and GUI projection coverage.

## 8. First milestone and migration risks

The first reliability milestone is
[S1: evidence-backed bug fixing](../roadmap/agent-harness.md#s1-evidence-backed-bug-fix-first-reliability-milestone):
submit a goal, inspect/edit, execute a relevant check, reject unsupported
completion, display evidence and preserve it across reload.

Primary risks to design around:

1. Adding a green GUI badge driven by the same old turn-completed event.
2. Persisting prose without task/evidence schemas and calling it durable work.
3. Removing TUI before migrating still-useful acceptance coverage.
4. Renaming permission modes without retaining explicit authority constraints.
5. Moving core wholesale into a new runtime crate rather than extracting owners.
6. Promising rollback for shell/MCP effects that were never captured.
7. Trusting a green check after repository/configuration changes invalidate it.

The roadmap addresses these risks before adding memory, more model roles or
advanced refactoring. Stronger prompts help, but are not the enforcement layer.
