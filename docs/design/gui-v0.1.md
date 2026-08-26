# harness-gui — Desktop App Design (v0.1)

Status: **design for review** · Owner decisions locked: Tauri 2 · macOS-only first · MVP includes sessions + settings.
Frontend note: v0.1 shipped on **React 19 + Vite** (rebuild from the original Svelte 5 scaffold); store/rune semantics map 1:1.

## 1. Goals

Give non-terminal users the full harness experience in a native-feeling macOS app,
while the TUI remains frozen as the keyboard-only power-user client.

Non-goals (v0.1): Windows/Linux packaging, image pasting, multi-project windows,
remote/server mode, plugin marketplace.

## 2. Architecture

```
┌─────────────────────────── harness-gui ───────────────────────────┐
│  Svelte 5 frontend (ui/)                                          │
│    stores: transcript, approvals, sessions, settings, usage       │
│         ▲ events (appEvent)          │ commands (invoke)           │
├─────────┼────────────────────────────┼────────────────────────────┤
│  src-tauri/ (Rust shell)                                           │
│    GuiState { handle: AgentHandle, cfg, session_meta }             │
│    forwarder task: EventRx → window.emit("appEvent", …)            │
│    #[tauri::command] handlers → AgentHandle methods                │
├────────────────────────────────────────────────────────────────────┤
│  harness-core (unchanged brain)                                    │
└────────────────────────────────────────────────────────────────────┘
```

Rules inherited from the TUI: the shell never touches tools directly;
everything flows through `AgentHandle` commands and `EventRx` events.

### 2.1 Tauri commands (frontend → Rust)

| Command | Args | Maps to |
|---|---|---|
| `submit` | text | `handle.submit` |
| `approve` | id, decision(once/session/persist) | `handle.approve` |
| `deny` | id | `handle.deny` |
| `abort` | — | `handle.abort` |
| `set_mode` | mode | `handle.set_mode` |
| `set_model` | model id | `handle.set_model` |
| `compact` | — | `handle.compact` |
| `notes` | — | `handle.request_notes` |
| `shutdown` | — | `handle.shutdown` |
| `list_sessions` | — | `session::list_sessions` |
| `read_session` | path | `session::read_events` |
| `delete_session` | path | fs delete (+confirm in UI) |
| `start_session` | {resume_path?} | spawn_with_recorder(+replay), swaps GuiState.handle |
| `get_config` | — | layered `Config` (redacted) + pricing + MCP table |
| `save_permission_rule` | rule | `config::persist_bash_rule` |
| `save_general` | {model?, base_url?, max_context_tokens?, review?} | `config::persist_general` (+ hot set_model) |
| `set_cost_override` / `remove_cost_override` | model, usd/mtok | `config::set_cost_override` / `remove_cost_override` |
| `list_mcp_servers` | — | resolved MCP server table |
| `test_mcp_server` | name | spawn + handshake + tools/list |
| `list_project_files` | query | gitignore-lite walk for the @file picker |

### 2.2 Events (Rust → frontend)

Existing `agent::Event` serialized as `appEvent` payloads, plus new ones:
`sessionChanged{ulid}` after `start_session`, `toolOutputDelta{id, chunk}`
(once core emits them).

## 3. Core additions required

1. `Event::ToolOutputDelta { call_id, chunk }` — bash tool streams stdout tail
   lines while running (also benefits the TUI later).
2. `session::delete_session(path)` helper.
3. `Config` exposure of resolved values for the Settings UI (already redacted-safe).
No agent-loop changes.

## 4. Screens

### 4.1 Main window (three zones)

```
┌──────────┬────────────────────────────────────────────┐
│ SIDEBAR  │ TRANSCRIPT (scroll)                        │
│ New task │ … message cards …                          │
│ ─ Today  ├────────────────────────────────────────────┤
│ …        │ COMPOSER                                   │
│ ─ Older  └────────────────────────────────────────────┘
│ Search   [status pill overlays bottom-right]
```

- **Sidebar**: `New task` button; sessions grouped by day (Today / Yesterday /
  dates); each shows ULID-short + first-user-msg preview + relative time;
  context menu: Open / Delete. Bottom: `Settings` gear.
- **Transcript**: vertical scroll of message cards (types in §4.2). Auto-follow
  while streaming unless user scrolled up; `↓ N new` jump pill when detached.
- **Composer**: auto-growing textarea (Shift+Enter newline), Send button +
  `⏎`; Stop button replaces Send while streaming; slash-command popup
  (`/compact`, `/notes`) filtered as typed; `@file` picker — typing `@`
  opens a filtered project-file list (gitignore-lite walk via
  `list_project_files`), Enter inserts the relative path; token + cost
  meter right-aligned from UsageUpdated events (cost from resolved
  pricing; tokens only for unknown models). `⌘K` opens a command palette
  mixing actions (new task, compact, notes, rewind, settings, mode,
  model presets) with recent sessions.

### 4.2 Message card types

| Type | Rendering |
|---|---|
| user | Right-accent bubble, plain text |
| assistant | Markdown (GFM: code blocks w/ highlight, lists, tables) |
| thinking | Dim block; streams live; auto-collapses to `✻ thought (N chars)`; click header toggles body |
| tool-call | Card: icon+name+args preview; states: ⟳ running (elapsed timer + live stdout tail, last ~10 lines, streamed via `ToolOutputDelta` from the bash drain) → done ✓/✗ collapsed to one line (summary + duration); click expands full output (monospace); spilled-output link opens temp file path |
| approval | Highlighted card: tool name, args table, unified diff (syntax-highlighted; collapsed above 15 lines with a toggle), four buttons `Approve once / Session rule / Persist rule / Deny`, persisted-rules hint; buttons disabled outside project scope for persist |
| thinking | Dim block; streams live with char count; auto-collapses to `✻ thought (N chars)`; click header toggles the retained body |
| notice/error | Centered dim / red text |

### 4.3 Permission modes

Dropdown in composer status area: `normal · auto-accept edits · plan`
(mirrors TUI Shift+Tab; drives `set_mode`). Mode badge also shown top-right
of transcript.

### 4.4 Sessions & settings

- Resume flow: pick session in sidebar → `start_session{resume_path}` replays
  JSONL into a fresh loop (same semantics as TUI).
- **Settings window** (tabbed):
  - General: model id, base URL, max context tokens, review toggle.
  - Permissions: rule list CRUD (scoped syntax `tool pattern`), writes via
    `persist_bash_rule`-style helpers into `.harness/config.toml`.
  - MCP servers: table of name/command/args with Test button (calls tools/list).
  - Cost: per-model USD/MTok overrides (falls back to built-in table).

## 5. Frontend architecture

- React 19 + Vite (TypeScript), module-level external stores consumed via
  `useSyncExternalStore`: `events.ts` (transcript/busy/usage/mode/draft),
  `configStore.ts` (resolved config + pricing); `commands.ts` wraps invoke.
- Rendering: markdown via `react-markdown` + `remark-gfm`; code blocks and
  unified diffs via `react-syntax-highlighter` (Prism, one-dark).
- Store logic is unit-tested with vitest (`src/lib/*.test.ts`).
- All provider/model knowledge stays in Rust; frontend is presentation-only.

## 6. Edge cases

| Case | Behavior |
|---|---|
| Stream drops mid-turn | Error card + Retry re-submits last user msg |
| rust-analyzer unavailable | Diagnostics fall back to cargo-check backend (existing) |
| Oversized tool output | Card links to spill file (`open` reveals in Finder) |
| App killed mid-turn | Session replay on next launch (JSONL) |
| Unknown model for cost | Meter shows tokens only |

## 7. Milestones & acceptance

M0 scaffold & plumbing — window opens; submit echoes streamed tokens; abort works.
M1 chat parity — all §4.2 card types incl. live tool tails + approvals + modes; reviewer findings visible.
M2 sessions & settings — sidebar CRUD/resume, settings tabs writing config.
M3 polish & package — keyboard map, icons, `tauri build` produces signed-ad-hoc .app.

Each milestone: `cargo test` green, `pnpm check`/`vitest` green, manual QA
script appended to `docs/ROADMAP.md`.

## 8. TUI policy

Frozen at v1.1.x: bugfixes only, no features. README marks TUI as the
keyboard-native client; GUI becomes the default recommendation.
