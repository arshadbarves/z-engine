# v2 engine architecture

Status: design for the v2 rewrite (branch `v2`). The crate layout and
dependency rules are in [AGENTS.md](../../AGENTS.md#v2-structure-branch-v2-authoritative-for-the-new-crates);
phase progress is in [status](../status.md).

The model supplies judgment; the engine supplies authority, execution,
evidence and durability. Every executable capability (tools, MCP, hooks,
checks) goes through one gate, and every observable change is an event.

## Runtime model

```mermaid
flowchart LR
  guiCmds["GUI commands"] --> actor["Session actor"]
  actor --> mainRun["Agent run: main"]
  mainRun --> gate["Gate: hooks, policy, approval"]
  gate --> broker["Broker: approvals, questions, plans"]
  broker --> actor
  gate --> exec["Tool execution"]
  exec --> agentTool["Agent tool"]
  agentTool --> childRun["Agent run: child or background"]
  childRun --> gate
  actor --> jobs["Jobs: background shells and agents"]
  actor --> events["Typed events to GUI"]
```

- **Engine** (process-wide): paths, session store, model catalog, web client,
  a `ClientFactory` (tests inject a scripted model), and an event sink
  (`Arc<dyn Fn(EventEnvelope)>`) shared by every session.
- **Session**: `SessionCore` is shared state behind short-lived locks
  (settings, policy, transcript state, session log, broker, jobs, tools).
  The **session actor** task owns the command channel and never blocks on
  the model or a tool: turns run in their own tasks and report back through
  an internal channel. Approvals, questions and plan reviews wait on the
  broker, so steering, mode changes and cancel keep working meanwhile.
- **Agent run**: one loop per agent (main or subagent), parameterized by an
  agent definition (prompt, tool filter, model, permission mode), a
  transcript sink (main: session log and state; subagent: its own
  `agents/<id>.jsonl`), a root directory (project or worktree), a depth,
  and a cancellation token.

Cancellation is a tree: session > turn > agent run > tool call. `Cancel`
and `Interrupt` cancel the foreground turn and its foreground subagents;
background jobs have tokens under the session and survive.

## One round of an agent run

1. Check cancellation, the per-run turn budget (`agents.max_turns`) and the
   session cost cap.
2. Relieve context pressure: above 50% of the window, clear old tool results
   (spilling originals to artifacts); above `context.compact_at_percent`,
   summarize older history with the `fast` model (`PreCompact` hook first).
3. Build the request: cache-stable system sections (base prompt,
   environment, instructions, skills, output style), filtered tool specs,
   the working transcript, cache breakpoints on the last two user messages,
   thinking/effort, and pending reminders appended to the last user message.
4. Stream the response: text/thinking deltas become events; `Retrying`
   becomes a `retrying` event; a context-overflow error forces compaction
   and one retry.
5. Record the assistant message and usage (per-agent and session cost from
   the catalog).
6. No tool calls: run the **stop boundary** (below). Otherwise execute the
   batch and append one user message holding every tool result (in call
   order), queued steering messages, and reminders (todo nudges, files
   changed externally, finished jobs, nested instructions).

Every `tool_use` always receives a `tool_result`, including on cancellation
("cancelled by user"), so a transcript is valid for any provider on resume.

### Tool batch

For each call, in order:

1. Unknown tool, malformed JSON or invalid input -> error result.
2. `PreToolUse` hooks (regex matcher on the tool name) may block, rewrite
   the input, or force allow/deny/ask.
3. `tool.action(input)` -> `policy.decide(tool, action, mode)`:
   `Allow` runs; `Deny` returns the reason; `Ask` becomes an approval
   request. All asks in a batch are requested together; the GUI may answer
   them in any order. `AllowSession` adds a session rule; `AllowProject`
   also persists the rule to `.z-engine/settings.local.toml`; `Deny` returns
   the user's feedback to the model.

Execution keeps call order: neighbouring calls that are concurrency-safe
run together; any other call is a barrier and runs alone. Each call emits
`toolStarted`, streamed `toolProgress`, and `toolFinished`, then runs
`PostToolUse` hooks (extra context or feedback is appended to the result).
Writes mark the run as mutated, refresh read tracking, and queue nested
`AGENTS.md`/rules reminders for newly touched directories.

### Stop boundary

When the model ends a response without tool calls:

1. `Stop` (main) or `SubagentStop` hooks may block with a reason; the run
   continues with that reason (at most 5 hook continuations per turn).
2. Steering messages that arrived meanwhile continue the run.
3. Main agent only, when files changed this turn: `verification.mode`
   `auto` runs the configured checks and feeds failures back; `strict`
   additionally continues until checks pass. Both are bounded by
   `verification.max_continuations`; `report` only computes the badge.
4. Otherwise the run ends. The turn records its outcome, usage, cost and
   verification badge (`Verified` / `Unverified` / `Failed` /
   `NotApplicable`).

## Tools contract (`z-engine-tools`)

```rust
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> String;        // from z-engine-prompts::tools
    fn input_schema(&self) -> Value;
    fn is_read_only(&self, input: &Value) -> bool;
    fn is_concurrency_safe(&self, input: &Value) -> bool;
    fn action(&self, input: &Value, ctx: &ToolCtx) -> Action;       // z_engine_policy::Action
    fn title(&self, input: &Value, ctx: &ToolCtx) -> String;        // "Edit src/main.rs"
    async fn preview(&self, input: &Value, ctx: &ToolCtx) -> Option<Preview>;
    async fn call(&self, input: Value, ctx: &ToolCtx) -> Result<ToolOutput, ToolError>;
}
```

`ToolCtx` is the per-call capability bundle: session/agent/call ids, root
directory and additional directories, the agent's persistent shell cwd,
a cancellation token, the agent's `FileTracker` (read-before-edit), shared
`PathLocks`, shell and environment policy, web client and options, a
progress sink, an artifact spill function, limits, whether the model has
vision, and `Ports`: engine services behind traits so tools never depend on
the engine:

- `AgentPort`: agent catalog, spawn (foreground/background/resume), apply a
  worktree agent's changes.
- `InteractionPort`: ask structured questions, propose a plan, update todos.
- `JobPort`: spawn a background shell, read job output, kill a job.
- `SkillPort`, `CheckPort` (list/run checks with evidence), `LspPort`,
  `McpPort` (call tools, list/read resources), `SideModelPort` (WebFetch
  extraction with the `fast` model).

## Subagents and jobs

- Definitions: built-ins (`general`, `explore`, `plan`, `review`, `verify`)
  shipped as markdown in `z-engine-prompts`, plus custom definitions from
  `.z-engine/agents` and `.claude/agents`. Tools are filtered by the
  definition; `AskUserQuestion` and `ExitPlanMode` are never given to
  subagents; `Agent` is removed at the depth limit.
- A subagent's permission mode is its definition's mode, else the parent's.
  Its approvals appear in the GUI labelled with its agent id.
- Foreground agents return their final message (plus usage and changed
  files) as the `Agent` tool result. Background agents return a job id at
  once; completion queues a reminder for the parent. `resume` continues a
  finished agent from its stored transcript.
- `isolation: worktree` runs the agent in `.z-engine/worktrees/<agent>` on a
  new branch. At the end its changes are committed and summarized; the
  parent (`ApplyAgentChanges`) or the user (GUI) applies them with a
  three-way merge or discards them.
- Jobs unify background shells and background agents: `JobOutput` reads
  new output, `JobKill` stops, `jobUpdated` events feed the jobs panel.

## Persistence

`z-engine-store` keeps one directory per session: `log.jsonl` (records:
messages, turns, todos, plans, questions, approvals, agents, checks,
checkpoints, compactions, rewinds, mode/model/title changes, usage),
`meta.json`, `agents/<id>.jsonl`, and `artifacts/`. Replay rebuilds the
display transcript and the model's working set; a turn that started but
never finished is reported as `Interrupted`. v1 files are imported on
first open and left untouched.

Before each user turn the engine snapshots the working tree into a shadow
git repository (outside the project; captures shell-made changes). Rewind
restores code, conversation, or both to the state before any user message.

## Settings files

v2 reads `settings.toml` files and never modifies v1 `config.toml` files,
so a v1 build keeps working side by side. Layers, lowest first: defaults,
`~/.config/z-engine/settings.toml`, `<project>/.z-engine/settings.toml`,
`<project>/.z-engine/settings.local.toml` (gitignored), then environment
overrides. When a v2 file is missing, the v1 `config.toml` at that level is
imported: the user file is converted once into `settings.toml`; project
files are converted in memory and written to `settings.toml` only on the
first change made through the app.

## Hooks

Configured in settings (`[[hooks.<Event>]] matcher, command, timeout_secs`).
The command receives JSON on stdin (`session_id`, `transcript_path`, `cwd`,
`hook_event_name`, and event fields such as `tool_name`, `tool_input`,
`tool_response`, `prompt`, `stop_hook_active`). Exit 0 continues (stdout
becomes context for `UserPromptSubmit`/`SessionStart`); exit 2 blocks with
stderr as the reason; other codes warn. JSON stdout may set `decision`
(`block`/`approve`), `reason`, `continue: false` with `stopReason`, and
`hookSpecificOutput` (`permissionDecision`, `permissionDecisionReason`,
`updatedInput`, `additionalContext`). Project-defined hooks, MCP servers
and checks run only after the user trusts the workspace.
