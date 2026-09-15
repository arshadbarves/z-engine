# Z Engine

A GUI-only coding agent with a Tauri desktop app. You provide a task ("fix this
failing test"); Z Engine can inspect files, run commands, edit code, and request
approval for gated operations.

Development is moving toward a **GUI-first, evidence-backed agent harness**:
the model supplies cognition; the harness supplies engineering discipline.
The desktop app is the only product frontend. The first
[evidence-gated completion slice](docs/architecture/verification-completion.md)
supports Rust workspaces in the GUI. The
[supervised harness](docs/architecture/supervised-harness.md) adds bounded
continuation, evidence-grounded context packets, and read-only project
discovery. This is a bounded engineering harness, not AGI; durable Pause/Resume
and operation reconciliation remain planned.
See the [architecture](docs/architecture/agent-harness.md),
[current-state assessment](docs/architecture/current-state.md), and
[vertical-slice roadmap](docs/roadmap/agent-harness.md).

## Install

Download the desktop installer for your operating system from
[GitHub Releases](https://github.com/arshadbarves/z-engine/releases):

- **macOS:** choose Apple Silicon or Intel, open the `.dmg`, and move Z Engine
  to Applications.
- **Windows:** run the `.exe` or `.msi` installer.
- **Linux:** install the `.deb`/`.rpm` package or make the AppImage executable
  and launch it.

Push or merge to the `release` branch (or run **Actions → release** by hand)
to build the desktop app for macOS (Apple Silicon and Intel), Windows, and Linux.
Installers and updater artifacts (`latest.json` + `.sig` files) are attached to the
GitHub Release matching the version in
`crates/z-engine-gui/src-tauri/tauri.conf.json`. The desktop app can then
download, install, and restart in-place via **Update & Restart** (requires
`TAURI_SIGNING_PRIVATE_KEY` / password secrets in the repo).

Configure a provider in Settings (OpenRouter by default) or use a local
OpenAI-compatible server. Install the toolchains needed by your projects;
Rust verification requires Cargo and a Rust toolchain. Optional: `ripgrep`
(grep falls back to pure Rust), `rust-analyzer`
(LSP tools fall back to tree-sitter/`cargo check`).

## Desktop quick start

1. Launch Z Engine and connect your provider in **Settings**.
2. Open a project folder and start a chat.
3. Describe the task, inspect proposed changes, and approve gated operations
   in the desktop approval dialog.
4. Reopen saved conversations from the sidebar to continue your work.

There is no standalone terminal or headless product. The agent can still run
shell commands through its permission-gated `bash` tool.

## Develop the desktop app

Requirements: Rust stable (≥1.85), Node.js LTS and npm, plus the
[Tauri platform prerequisites](https://v2.tauri.app/start/prerequisites/)
for your OS (including WebKitGTK on Linux).

From the repository root:

```bash
npm ci --prefix crates/z-engine-gui/ui
cd crates/z-engine-gui
node ui/node_modules/@tauri-apps/cli/tauri.js dev
```

On macOS/Linux, [`./scripts/dev-gui.sh`](scripts/dev-gui.sh) from the repository
root also launches the app with Vite hot reload. To build native installers on
the current OS, start again from the repository root:

```bash
npm run build --prefix crates/z-engine-gui/ui
cd crates/z-engine-gui
node ui/node_modules/@tauri-apps/cli/tauri.js build
```

These commands use Tauri's development tooling, not a Z Engine command-line
frontend. Release packaging requires the updater signing environment described
above. Run targeted checks during development:

```bash
cargo test -p z-engine-core --test verification_completion
npm test --prefix crates/z-engine-gui/ui
npm run check --prefix crates/z-engine-gui/ui
```

## Configuration

Ladder (lowest→highest): defaults < `~/.config/z-engine/config.toml` <
`<project>/.z-engine/config.toml` < env (`ZENGINE_MODEL`, `ZENGINE_BASE_URL`).
Provider/model selections can also be changed in the desktop app.

Missing `~/.config/z-engine/config.toml` and `auth.json` are created on
first launch. There is no fallback to the old `harness` paths.

```toml
model = "anthropic/claude-sonnet-4"
base_url = "https://openrouter.ai/api/v1"      # or http://localhost:11434/v1
max_context_tokens = 120000
review = true                                   # post-edit reviewer pass
max_task_continuations = 3                       # 0 disables; allowed range 0-10
task_report_view = "quiet"                      # quiet, compact, or detailed

[permissions]
allow = ["cargo test*", "git status"]           # bash prefix rules

[cost]                                          # optional calibration
# usd_per_mtok_input / usd_per_mtok_output are read from the built-in table;
# per-model overrides land with your provider config.

[mcp.servers.echo]
command = "python3"
args = ["scripts/mcp_echo_server.py"]
```

API key: set it in the GUI Settings page (stored in
`~/.config/z-engine/auth.json`), or `ZENGINE_API_KEY`. It never lives in
config.toml. MCP servers, permissions, and other options are also edited
from Settings.
`task_report_view` controls task-report information density; `quiet` is the
default and keeps verification details available on demand.

OpenCode Zen is available as an OpenAI-compatible provider at
`https://opencode.ai/zen/v1`. Free `/chat/completions` models (`*-free`,
`big-pickle`) work without a key; paid models need a Zen account key or
`OPENCODE_API_KEY`. Models that only speak `/responses` or `/messages` are
not supported yet.

## Tools the model gets

`bash` (persistent cwd, env allowlist, timeout+kill of the process group,
output truncation to spill files) · `read_file` · `write_file` /
`edit_file` (exact → line-hint → fuzzy ladder, read-before-edit enforced) ·
`glob` · `grep` (ripgrep fast-path) · `update_context_notes` · `task`
(isolated read-only sub-agent) · `go_to_definition` / `find_references` /
`lsp_diagnostics` (rust-analyzer; falls back to outlines + `cargo check`)
plus any MCP externals you register.

`run_verification` records typed Cargo test/build evidence through the normal
approval gate. `assess_completion` records requirement coverage, not a success
claim. A verified Complete outcome requires a current full-workspace test pass,
executed tests, intact evidence artifacts, coverage of the original goal, and
durable task recording. Targeted checks remain useful during investigation but
cannot alone satisfy the S1 completion gate.

## Bounded continuation and support limits

When observed edits or checks leave a supported task incomplete, the supervisor
can request another model round under the same task and original requirement.
`max_task_continuations` defaults to 3 (range 0-10; 0 disables continuation)
and is captured at session startup. Repeated/cyclic states, exhausted budgets,
permission denials, cancellation, and execution/evidence blockers stop further
automatic attempts. A model's closing response is not verified completion.
Read-only responses do not automatically loop just because they lack tests;
premature answers before observed edits/checks are not yet automatically replanned.

Language-neutral file/search/edit/shell tools remain available. Project profiles
can discover Cargo, Node, Python, Go, Gradle, Maven, .NET, CMake, and Make markers
and statically justified check suggestions. Discovery does not execute commands,
prove a toolchain is installed, or provide semantic refactoring. Profiles with
insufficient configuration can remain marker-only or unsupported.

Typed verification and the Complete gate remain limited to the current
full-workspace Cargo profile. Discovering another ecosystem does not make its
suggested commands verified evidence. See the
[supervised harness contract](docs/architecture/supervised-harness.md) for
current boundaries and explicitly deferred capabilities.

## How it stays on budget

Provider-reported usage drives a meter (warn ≥80%, auto-compaction ≥92%):
old tool outputs elide to spill files, old prose is summarized into durable
context notes via a side-request, and `/compact` forces it on demand.

## Sessions

Every turn appends newline-delimited JSON events under
`~/Library/Application Support/z-engine/sessions/<ulid>.jsonl`
(macOS), `~/.local/share/z-engine/sessions` (Linux), or
`%APPDATA%\z-engine\sessions` (Windows). Crashes tear at most the last
line; reopening a saved conversation in the desktop sidebar replays it so you
can continue without a command-line session flag.

On Windows the GUI uses a native title bar. The `bash` tool prefers Git
Bash (`bash -lc`) when it is on `PATH`, otherwise `cmd.exe /C`. Install
[Git for Windows](https://git-scm.com/download/win) for POSIX commands.

## Architecture and engineering

The desktop product is composed from these crates:

- [z-engine-provider](crates/z-engine-provider): provider HTTP/SSE and wire types.
- [z-engine-runtime](crates/z-engine-runtime): pure bounded supervision contracts
  and decisions; no provider or UI dependency.
- [z-engine-context](crates/z-engine-context): provider-independent bounded
  packets separating task data, observed evidence, and unverified model notes.
- [z-engine-project](crates/z-engine-project): read-only manifest/profile
  discovery and check suggestions; no command execution.
- [z-engine-core](crates/z-engine-core): integration with the agent loop, tools,
  permissions, context, sessions, verification, and LSP/MCP; no UI dependencies.
- [z-engine-gui](crates/z-engine-gui): Tauri shell and Svelte 5 desktop frontend.

Start here:

- [Supervised harness](docs/architecture/supervised-harness.md): implemented
  bounded continuation, context/discovery boundaries, and supported profile limits.
- [Current-state assessment](docs/architecture/current-state.md): code-grounded
  capabilities, limitations, and reusable seams.
- [GUI-first architecture](docs/architecture/agent-harness.md): proposed
  boundaries, dependencies, and staged extraction.
- [Task/runtime contracts](docs/architecture/task-runtime.md): completion,
  verification, Stop/Pause, recovery, questions, and durable state.
- [Implemented S1 contract](docs/architecture/verification-completion.md):
  the Rust verification tools, completion gate, persistence and current limits.
- [Engineering & Coding Style Guide](docs/engineering/style-guide.md):
  conventions for new and changed code.
- [Vertical-slice roadmap](docs/roadmap/agent-harness.md): acceptance gates and
  planned coverage of the twenty product requirements, not delivered guarantees.
- [Structure contract](AGENTS.md) and [GUI UI guide](docs/design/gui-ui-guide.md):
  current layout and frontend rules.

Earlier [specifications](docs/superpowers/specs),
[deviations](docs/deviations.md), and [roadmap evidence](docs/ROADMAP.md) are
historical records, not proof of the proposed harness guarantees.
