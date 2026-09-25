# Z Engine

A coding agent with a Tauri desktop app. You describe a task ("fix this
failing test"); Z Engine reads files, runs commands, edits code, and asks for
approval before gated operations. The model supplies judgment; the engine
supplies authority, execution, evidence and durability: every tool, hook and
check goes through one permission gate, and every observable change is a
typed event the GUI renders.

Version 2 is a from-scratch rewrite. Its design is in
[v2 engine architecture](docs/architecture/v2-engine.md). The desktop app is
the only product frontend; there is no terminal or headless product.

Highlights:

- **Multi-agent orchestration:** built-in and custom subagents run in
  parallel, nested, in the background, or resumed, each with its own model,
  tools and permission mode; an agent can work in its own git worktree and
  hand back changes you apply or discard.
- **Claude Code-compatible tools, commands, hooks and `.claude/` folders**,
  so existing agents, commands, skills and `CLAUDE.md` files work as-is.
- **Steering and interrupts:** messages sent while the agent works are
  injected at the next step; nothing is dropped while an approval waits.
- **Evidence-backed badges:** every turn that changes files is marked
  Verified, Unverified or Failed from the checks that actually ran.
- **Durable checkpoints:** code (including shell-made changes) and
  conversation can be rewound to any prompt; sessions survive crashes.
- **Native Anthropic caching and extended thinking**, OpenAI-compatible
  providers, fallback models, and per-agent cost tracking.
- **Optional sandbox** that confines shell commands to the workspace.

## Install

Download the desktop installer for your operating system from
[GitHub Releases](https://github.com/arshadbarves/z-engine/releases):

- **macOS:** choose Apple Silicon or Intel, open the `.dmg`, and move Z Engine
  to Applications.
- **Windows:** run the `.exe` or `.msi` installer.
- **Linux:** install the `.deb`/`.rpm` package or make the AppImage executable.

The app updates itself in place (**Update & Restart**) from signed release
artifacts. Push or merge to `release` (or run **Actions → release**) to build
installers and updater artifacts for the version in
`crates/z-engine-gui/src-tauri/tauri.conf.json`; this needs the
`TAURI_SIGNING_PRIVATE_KEY` secrets.

Optional tools: `git` (code checkpoints, rewind, diffs, worktrees), `ripgrep`
(Grep falls back to a pure-Rust search).

## Quick start

1. Launch Z Engine and connect a provider in **Settings → Providers**
   (OpenRouter by default; Anthropic, OpenAI, OpenCode Zen, or any local
   OpenAI-compatible server).
2. Add a project folder and start a chat.
3. Describe the task. Approve or deny gated actions on the approval cards;
   "always" answers become session or project rules.
4. Reopen chats from the sidebar; v1 chats are imported on first open.

In the composer: `@` mentions files and agents, `/` lists commands, `!cmd`
runs a shell command in the project, `#note` saves a memory, images can be
pasted or dropped, and messages sent while the agent works are queued as
steering.

## Develop

Requirements: Rust stable (≥1.85), Node.js LTS and npm, and the
[Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) for your OS.

```bash
npm ci --prefix crates/z-engine-gui/ui
./scripts/dev-gui.sh              # Tauri dev with Vite hot reload
```

Installers for the current OS:

```bash
npm run build --prefix crates/z-engine-gui/ui
cd crates/z-engine-gui && node ui/node_modules/@tauri-apps/cli/tauri.js build
```

Checks (all required before a commit; see [AGENTS.md](AGENTS.md)):

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
npm test --prefix crates/z-engine-gui/ui
npm run check --prefix crates/z-engine-gui/ui
npm run lint --prefix crates/z-engine-gui/ui
```

`cargo test -p z-engine-protocol` (and `-p z-engine-config`) regenerate the
TypeScript mirror in `crates/z-engine-gui/ui/src/lib/protocol/`; commit it.

## Configuration

Settings are TOML files layered lowest first:

1. built-in defaults
2. `~/.config/z-engine/settings.toml` (user; `ZENGINE_CONFIG_DIR` overrides the folder)
3. `<project>/.z-engine/settings.toml` (shared with the repository)
4. `<project>/.z-engine/settings.local.toml` (personal, gitignored)
5. environment: `ZENGINE_MODEL`, `ZENGINE_BASE_URL`, `ZENGINE_PROVIDER`, `ZENGINE_SHELL`

```toml
schema = 2

[model]
main = "anthropic/claude-sonnet-4.5"
fast = "..."                        # titles, summaries, quick subagents

[provider]
base_url = "https://openrouter.ai/api/v1"

[permissions]
mode = "default"                    # default | acceptEdits | plan | bypass
allow = ["Bash(cargo test:*)"]
deny = ["Read(./.env)"]

[verification]
mode = "report"                     # off | report | auto | strict

[mcp.servers.echo]
command = "python3"
args = ["scripts/mcp_echo_server.py"]

[[hooks.PreToolUse]]
matcher = "Bash"
command = "./scripts/check-command.sh"
timeout_secs = 60
```

Everything is also editable in Settings, which writes the layer you choose.
v1 `config.toml` files are never modified: the user file is converted once
into `settings.toml`; a project's v1 file is read in memory and written to
`settings.toml` on the first change made through the app.

API keys live in `~/.config/z-engine/auth.json` (the v1 format, shared with
v1), set from Settings or through `ZENGINE_API_KEY` / the provider's own
variable (`OPENROUTER_API_KEY`, `ANTHROPIC_API_KEY`, `OPENAI_API_KEY`,
`OPENCODE_API_KEY`). OpenCode Zen (`https://opencode.ai/zen/v1`) serves its
free models without a key.

Until you trust a workspace, its project settings can only make things
stricter: hooks, MCP servers, checks, permission modes and allow rules,
shell, provider, web and language-server settings come from your user
settings, while the project's deny and ask rules still apply. A banner in
the chat asks when a project sets any of them, and
**Settings → Advanced → Workspace** changes it later.

## Tools

Named and shaped like Claude Code's: `Read`, `Write`, `Edit`, `MultiEdit`,
`NotebookEdit`, `Glob`, `Grep`, `Bash` (persistent cwd, background jobs),
`JobOutput`, `JobKill`, `WebFetch`, `WebSearch`, `TodoWrite`,
`AskUserQuestion`, `ExitPlanMode`, `Skill`, `Agent`, `ApplyAgentChanges`,
`Verify`, `LSP`, `ListMcpResources`, `ReadMcpResource`. Edits require a fresh
read of the file; every call passes the permission gate.

## Agents, commands and hooks

Built-in agents: `general`, `explore`, `plan`, `review`, `verify`. Custom
agents, commands, skills, rules and output styles are markdown files with YAML
frontmatter in `~/.config/z-engine/` or `<project>/.z-engine/`
(`agents/`, `commands/`, `skills/<name>/SKILL.md`, `rules/`,
`output-styles/`); `.claude/` folders and `CLAUDE.md` are read too. Agents run
in the foreground or background, optionally in their own git worktree.

Hooks (`SessionStart`, `UserPromptSubmit`, `PreToolUse`, `PostToolUse`,
`Stop`, `SubagentStop`, `PreCompact`, `Notification`, `SessionEnd`) receive
JSON on stdin; exit 2 blocks with stderr as the reason. Details are in the
[architecture](docs/architecture/v2-engine.md#hooks).

Slash commands come from four sources: session commands (`/compact`,
`/context`, `/cost`, `/status`, `/model`, `/mode`, `/effort`, `/mcp`,
`/todos`, `/doctor`, `/add-dir`, `/remember`), built-in prompt commands
(`/init`, `/review`, `/security-review`, `/commit`), your own markdown
commands, and MCP prompts (`/mcp__<server>__<prompt>`). Custom commands
support `$ARGUMENTS` and `$1`..`$9`, `@path` file inclusion, inline
`` !`cmd` `` output, and frontmatter `allowed-tools` (granted for that turn
only) and `model` (that turn's model).

## MCP and language servers

MCP servers run over stdio or streamable HTTP. Their tools appear as
`mcp__<server>__<tool>` behind the same permission gate; resources are read
with `ListMcpResources`/`ReadMcpResource`; large tool sets are loaded on
demand. The `LSP` tool offers definitions, references, hover, symbols, call
hierarchy, diagnostics and rename previews through rust-analyzer,
typescript-language-server, pyright/basedpyright, gopls or clangd when they
are installed, and edits report new compile errors right away.

## Sandbox

With `[shell.sandbox] enabled = true`, shell commands, background jobs and
checks run under macOS `sandbox-exec` or Linux `bwrap`: writes are limited
to the workspace, extra directories, temp and tool caches; hook and settings
files stay read-only; the network can be blocked (`allow_network`). With
`auto_allow`, sandboxed commands run without approval prompts, while deny
and ask rules still apply. Platforms without a backend run commands
unconfined and keep asking.

## Verification

`verification.mode` decides what happens when the agent stops after changing
files: `report` only shows the badge, `auto` runs the configured checks and
feeds failures back, `strict` keeps going until they pass (bounded by
`verification.max_continuations`). Each turn ends `Verified`, `Unverified`,
`Failed` or `NotApplicable`.

## Sessions

Each chat is a folder under the data directory's `sessions/`
(`~/Library/Application Support/z-engine` on macOS, `~/.local/share/z-engine`
on Linux, `%APPDATA%\z-engine` on Windows; `ZENGINE_DATA_DIR` overrides it):
`log.jsonl`, `meta.json`, subagent transcripts, and spilled artifacts. Before
each prompt the working tree is snapshotted into a shadow git repository
outside the project, so a chat can rewind code, conversation, or both. The
app log is `z-engine-gui.log` in the same data directory.

## Documentation

Read it on the documentation website,
[arshadbarves.github.io/z-engine](https://arshadbarves.github.io/z-engine/),
or here in the repository:

- [User guide](docs/user-guide/README.md): everything about using the app,
  from getting started to the full settings reference.
- [How Z Engine works](docs/how-it-works/README.md): each feature and crate
  in plain words first, then the mechanism, then developer detail.
- [Documentation home](docs/README.md): which document to read for what.

## Architecture

[AGENTS.md](AGENTS.md) is the structure contract (crates, dependency rules,
file budget, and the [documentation contract](docs/AGENTS.md)). Read next:
[v2 engine](docs/architecture/v2-engine.md),
[GUI UI guide](docs/design/gui-ui-guide.md), and the
[style guide](docs/engineering/style-guide.md).
