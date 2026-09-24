# Changelog

All notable changes to the **Z Engine** project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- **User guide** (`docs/user-guide/`): complete documentation for using the
  app, from getting started to the settings reference and troubleshooting.
- **How Z Engine works** (`docs/how-it-works/`): every feature and crate
  explained in plain words first, then the mechanism, then developer detail,
  with a glossary.
- **Documentation contract** (`docs/AGENTS.md`), a `docs-maintainer`
  subagent, and a CI link check, so feature changes update the docs in the
  same change.

---

## [2.0.0] - 2026-09-24

A from-scratch rewrite of the engine, tools, persistence and the desktop
runtime. See the [v2 engine architecture](docs/architecture/v2-engine.md).

### Added
- **Multi-agent orchestration**: built-in agents (`general`, `explore`,
  `plan`, `review`, `verify`) and custom agents from markdown files run in
  parallel, nested, in the background, or resumed, each with its own tools,
  model and permission mode, per-agent usage and cost, and an agent tree and
  jobs panel in the app.
- **Worktree isolation**: an agent can work in its own git worktree; its
  changes are applied with a three-way merge (conflicts reported) or
  discarded.
- **Claude Code-compatible tools**: `Read` (images, PDFs, notebooks),
  `Write`, `Edit`, `MultiEdit`, `NotebookEdit`, `Glob`, `Grep`, `Bash` with
  background jobs, `JobOutput`, `JobKill`, `WebFetch`, `WebSearch`,
  `TodoWrite`, `AskUserQuestion`, `ExitPlanMode`, `Skill`, `Agent`,
  `ApplyAgentChanges`, `Verify`, `LSP`, MCP resources and `LoadMcpTools`.
- **Planning and interaction**: todo strip, structured questions, and plan
  mode with an editable plan approval card.
- **Steering and interrupts**: messages sent while the agent works are
  injected at the next step; Cmd+Enter interrupts and redirects.
- **Hooks** for every agent event with a Claude Code-compatible JSON
  protocol (block, rewrite input, add context, force continuation).
- **Slash commands**: session commands, `/init`, `/review`,
  `/security-review`, `/commit`, custom markdown commands with arguments,
  file inclusion, inline shell output and turn-scoped tool grants, and MCP
  prompts.
- **Verification badges**: checks are discovered for most ecosystems, runs
  are recorded as evidence, and each turn is marked Verified, Unverified or
  Failed; optional `auto` and `strict` modes.
- **MCP over stdio and streamable HTTP**, deferred loading for large tool
  sets, and a multi-language **LSP** integration with post-edit diagnostics.
- **Native Anthropic provider** with prompt caching and extended thinking;
  cache breakpoints for OpenRouter; fallback models; a models.dev catalog
  with cache-aware cost.
- **Durable checkpoints** in a shadow git repository (shell changes
  included) and rewind of code, conversation, or both.
- **Optional sandbox** for shell commands (macOS seatbelt, Linux
  bubblewrap) with opt-in auto-approval.
- **Workspace trust** for project-defined hooks, MCP servers and checks.
- Hierarchical instructions (`AGENTS.md`, `CLAUDE.md`, nested files,
  glob-scoped rules), a multi-language repo map, and compaction that can
  summarize inside long agentic turns.
- Settings screens for models, providers, permissions, hooks, agents and
  commands, MCP, verification, memory and advanced options, per scope.

### Changed
- Settings live in `settings.toml` (user, project, project-local) with
  allow/ask/deny permission rules such as `Bash(npm test:*)`.
- Sessions are stored as one folder per chat with subagent transcripts and
  artifacts.

### Migration
- v1 `config.toml` files are never modified: the user file is imported once
  into `settings.toml`, project files are read in memory until the first
  change made in the app.
- v1 chats are imported on first open; the original files stay untouched.
- API keys in `auth.json` keep working. Keys for custom endpoints are now
  only sent to their own host and may need to be entered again.

### Removed
- The v1 crates (`z-engine-core`, `z-engine-provider`, `z-engine-runtime`,
  `z-engine-project`) and the task-report views they fed.

---

## [1.4.6] - 2026-09-15

### Added
- **Procedural Apple-Grade Brand Icon & Logo**:
  - Bespoke procedural generator (`scripts/generate_icons.py`) mathematically constructing Apple-grade continuous squircle, liquid titanium kinetic Z geometry, radiant solar combustion core, machined chamfer highlights, and multi-stage drop shadows.
  - Complete multi-platform desktop asset suite: macOS `icon.icns`, Windows `icon.ico`, multi-resolution PNGs (`icon.png`, `icon-512.png`, `icon-256.png`, `128x128.png`, `64x64.png`, `32x32.png`, Windows Store & Square logos), `icon.svg`, and web `favicon.svg`.
  - In-app `LogoMark.svelte` updated with kinetic Z geometry and lightweight gradients for crisp rendering across all UI surfaces (TopBar, Sidebar, HomeScreen, About modal).
- **Supervised Task Runtime & Verification Architecture**:
  - Bounded task supervision, durable verification gate, evidence collection, and task report projection.
  - Refined GUI components including TaskReportCard, CheckEvidenceDetails, ActivityTabs, AppearanceSettings, process disclosure, and settings cards.
  - Specialized bounded crates: `z-engine-context`, `z-engine-runtime`, and `z-engine-project`.

---

## [1.4.5] - 2026-09-07

### Fixed
- **Read-Only Filesystem on macOS App Launch**: Fixed `os error 30` when connecting providers or saving settings from the GUI by persisting general settings to the user's global configuration (`~/.config/z-engine/config.toml`), and gracefully falling back to saved workspaces or user home directory on launch instead of the root filesystem (`/`).
- **Workspace Project Context Synchronization**: Automatically sync `ctx.project_root` when a new workspace is added in the GUI.

---

## [1.4.4] - 2026-09-05

### Added
- **Minimal Premium Chat Redesign**: Collapsed noisy auxiliary tool activity into subtle, single-line process strips with in-place Apple segmented inspector (All, Files, Searches, Terminal, Reasoning).
- **Floating Scrubber Timeline**: Unobtrusive right-rail conversation scrubber with clean prompt tooltips, zero turn numbers, and razor-sharp hairline borders.
- **Apple Frosted User Bubbles**: High-contrast dark bubbles with smooth hover-reveal micro-actions (copy, edit/revert) and image attachment previews.
- **Floating Composer Dock**: Unified toolbar controls with circular send/stop button and modular architecture meeting the file budget.
- **Settings Redesign**: Clean Apple preferences layout with dedicated Providers directory, connection modals, and permission safety controls.
- **Prompt Inspector**: Consolidated context meter eliminating duplicate token counters.

---

## [1.4.1] - 2026-09-01

### Fixed
- **Auto-Updater Signature Verification**: Updated `pubkey` in `tauri.conf.json` to match the Minisign signing key used in GitHub Actions release workflow.
- **Unified Version Architecture**: Centralized version management via `scripts/bump-version.sh` and dynamic version extraction in packaging and UI.

### Added
- **Integrated Changelog Viewer**: Added dynamic changelog fetching and markdown viewer inside Settings > About with offline embedded fallback.

---

## [1.4.0] - 2026-08-31

### Added
- **Master-Detail Workbench Changes**: Replaced accordion cards with a sleek Master-Detail review layout featuring a file explorer sidebar on the left and full-height syntax-highlighted diff viewer on the right.
- **Virtualized Diff Chunking**: High-performance line chunking and file pagination capable of rendering repositories with 10,000+ changed files smoothly.
- **Dual Diff Scopes**: Switch between **Current Session** net modifications and **Git Working Tree** uncommitted changes.
- **Fluid Docking Animations**: Added `0.24s cubic-bezier` slide-in and slide-out transitions on opening and closing side panels.
- **Docked Worktree Panel**: Migrated Git Worktree creation from a floating modal to a docked side container.
- **TopBar Redesign**: Search trigger converted to a compact icon button with centered draggable window titlebar badge.
- **Sidebar Marquee & Fade Blur**: Added right-edge gradient fade mask and hover marquee effect for long workspace titles.

### Fixed
- **Typography Descender Clipping**: Corrected line-height and vertical baseline alignment to prevent descender clipping (`g`, `y`, `p`, `q`, `j`) in file lists.
- **Floating Terminal Overlay**: Decoupled terminal popup from composer flex flow and fixed autocomplete dropdown keyboard scrolling.

---

## [1.3.0] - 2026-08-27

### Added
- **Custom Luxury Provider Picker**: Custom dropdown selector in settings for OpenRouter, Anthropic, OpenAI, and DeepSeek.
- **Live Memory & Headroom Stats**: Integrated real-time cache analytics and token headroom metrics.
- **Apple Fluid Physics**: Spring physics on dialogs and overlays.

---

## [1.2.0] - 2026-08-20

### Added
- **Multi-Workspace Management**: Support for switching and managing multiple git repositories from the sidebar.
- **Session Checkpoint & Rewind**: Tree-based session checkpointing and step-by-step turn revert.
- **Model Catalog Integration**: Live OpenRouter and provider model catalog fetching with context window stats.

---

## [1.1.0] - 2026-08-15

### Added
- **Tauri Desktop GUI Frontend**: Svelte 5 + Bits UI high-performance desktop client.
- **Dual Frontend Architecture**: Terminal UI (Ratatui) and Desktop GUI sharing unified `z-engine-core`.
- **Streaming Tool Approvals**: Interactive permissions engine with per-command rule persistence.
