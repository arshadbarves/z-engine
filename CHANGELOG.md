# Changelog

All notable changes to the **Z Engine** project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- **The pet**: a small creature, **Zen** by default, replaces the
  title-bar companion orb. Its face and posture show what the agent is
  doing (working, needs you, done, failed) across 34 moods, it gets sleepy after quiet
  minutes and greets you when the app opens (it wakes up on the splash,
  then flies into the title bar). At the **Lively** level it roams while
  nothing needs you: above **What should we work on?** on the project
  home, onto the composer's top edge, the sidebar footer and the open side
  panel's tab bar, and into the empty Inbox; it watches the composer while
  you type, naps after 3 minutes, rides back into the island while the
  agent works and hops in when something needs you. It walks along edges
  and hops between spots on springs. Drag it anywhere (it swings from your
  grip), throw it (a hard throw leaves it dizzy), click to boop it,
  double-click or right-click for its card.
- **Pet growth**, saved in `<data dir>/pet.json`: XP for completed turns
  (10), verified turns (15 more), applied helper worktrees (20) and the
  first of these each day (5 more, with a day streak). Levels change its
  shape (Seed, Sprout, Bloom, Star), unlock things to wear (sprout, scarf,
  headphones, star) and idle tricks (stretch, hop, spin, sparkle). At
  Lively a new level plays a short confetti celebration once nothing needs
  you.
- **Pet card**: level ring, name, level and stage, XP to the next level,
  streak, turns, verified and applied counts, what it wears, its tricks,
  and **Customize…**, **Stop roaming** / **Let it roam** and **Call back**.
- **Settings → Pet**: a preview with **Show its card**, **Name**, **Look**
  (Pearl, Mint, Sky, Lilac, Peach, Graphite), **Liveliness** and **Let it
  roam**; Settings search finds each one. **About & Updates** lists
  `pet.json`.
- New settings `[ui.pet]`: `name` (default `"Zen"`, up to 24 characters),
  `look` (default `"pearl"`) and `roam` (default `true`).
- First-run setup starts by meeting the pet (**Hi, I'm Zen**, a name and a
  look), keeps it above the setup card on every step, and asks in the work
  style step whether it roams.
- Command palette **Pet** actions: **Rename Zen**, **Change Zen's look**,
  **Show Zen's card**, **Stop Zen roaming** / **Let Zen roam** and **Call
  Zen back** (with your pet's name).
- **Side panel**: one panel beside the chat with **Changes**, **Plan**,
  **Agents** and **Context** tabs, shown or hidden from the title bar.
  Drag its edge to resize it (the width is kept), let it use the whole
  stage, and press Esc to step back; ← and → switch tabs. A plan waiting
  for review opens the Plan tab, and a new helper marks Agents with a dot.
- **Plan tab**: the plan waiting for review, with **Edit** / **Preview**,
  **Approve & auto-accept edits**, **Approve & ask before edits** and
  **Keep planning**, else the chat's last plan; the todo checklist shows
  **N of M done**.
- **Turn actions** under each answer: copy the answer, **Open changes** and
  **Rewind** (to before this prompt: code and conversation, conversation
  only, or code only).
- A finished turn's work folds into one line such as **Worked for 1m 12s ·
  read 6 files · ran 2 commands**, with failed and denied counts.
- **Compacting context…** shows while a summary is being written, before
  the **Context compacted** divider; new protocol event
  `compactionStarted { trigger }`.
- Double-clicking an empty part of the title bar, the sidebar's head, the
  side panel's head or the Settings bar maximizes or restores the window.

### Changed
- **Dark frosted redesign**: the app is always dark, over the dark HUD
  vibrancy on macOS (active even when the window is in the background) and
  dark Mica on Windows 11, solid on Windows 10 and Linux. Four layers (the
  window, the content sheet, glass for the sidebar card, side panel,
  composer and island, stronger glass for popovers, menus and dialogs) use
  one glass recipe with no gradients and a blue accent. Motion runs on
  springs and moves only position and opacity: the home, a chat and the
  Inbox fade up as you switch, and panels slide rather than resize. The
  sidebar is a floating card that slides away, with the macOS window
  buttons inside its head; its footer no longer has the model chip (pick
  the model in the composer, set the default in **Settings → Providers**).
- **Approvals and questions wait in the composer**, in place of the text
  box: the oldest approval first (it takes the focus unless you are typing
  elsewhere), else the oldest question, with **N more waiting after
  this**; your draft comes back once they are answered. A plan shows as a
  **Plan ready · Review** row in the transcript and opens the Plan tab.
- **Island**: it rests, shows live steps and notices, and while the chat
  needs you offers one **Approve**, **Answer** or **Review** button that
  goes to the waiting card. Clicking the island itself, amber or not,
  grows it into a card (the notice
  and its actions, the plan, the latest steps, rows for helpers, context
  and waiting chats, cost, recent warnings) instead of a sheet.
- The Changes and Agents panels and the full-window prompt inspector are
  the side panel's **Changes**, **Agents** and **Context** tabs. The
  Context tab keeps the ring map, outline and **Insights** docked and adds
  the reader when it uses the whole stage.
- **Rewind** moved from your message to the turn's actions, and copying the
  answer moved there too; your message keeps **Copy** on hover.
- Long chats draw the newest 30 turns and 20 more each time you scroll near
  the top (a click on the prompt rail draws the turns back to that prompt);
  older turns and code colors fill in only as they near the screen, so a
  1,000-turn chat scrolls smoothly and an idle window does almost no work.
- Dialogs share one layout (an optional icon, the title and a line under
  it, then the body, the actions last), and **Settings → Providers** groups
  the endpoint and model new chats start with under **In use**.
- `ui.companion` (**Lively**, **Calm**, **Off**) moved from **Settings →
  Appearance** to **Settings → Pet** as **Liveliness**; **Calm** now keeps
  the pet in the title bar, and **Off** still shows a small dot.
- Send and Stop in the composer are one button: while the agent works and
  the draft is empty, Send turns into Stop.
- Helper calls use one summary line with a tiny sprite of the pet, in the
  transcript's Agent card and in the Agents tab's rows.
- The prompt inspector's map is a ring chart of the context window; its
  legend filters the outline.
- The sidebar's selection highlight slides between rows; Settings opens as
  a sheet over the window; the project home without a project centres its
  empty state.
- The splash shows the pet instead of the Z mark and the name: it drops
  in, wakes up, breathes and blinks while the app loads, then flies into
  its place in the title bar (or above the setup card on a fresh install).
  With Reduce Motion it simply fades.
- The app icon, the favicon and the logo in **Settings → About & Updates**
  show the pearl pet glowing on a midnight squircle instead of the Z mark.

### Removed
- The title-bar companion orb and its **Companion** group in **Settings →
  Appearance** (the pet and **Settings → Pet** replace them).

### Fixed
- Glass stays blurred after the first-launch entrance animations.
- The idle pet no longer redraws every frame, for lower CPU and battery use
  while nothing happens.
- The Changes tab stacks the file list above the diff when the side panel
  is narrow.
- Esc closes the island's card before the side panel.
- A compaction divider at the end of a turn sits after the turn's actions.

## [2.1.0] - 2026-09-25

### Added
- **User guide** (`docs/user-guide/`): complete documentation for using the
  app, from getting started to the settings reference and troubleshooting.
- **How Z Engine works** (`docs/how-it-works/`): every feature and crate
  explained in plain words first, then the mechanism, then developer detail,
  with a glossary.
- **Documentation contract** (`docs/AGENTS.md`), a `docs-maintainer`
  subagent, and a CI link check, so feature changes update the docs in the
  same change.
- **Documentation website** at
  [arshadbarves.github.io/z-engine](https://arshadbarves.github.io/z-engine/):
  the user guide, how it works and the contributor docs, with search,
  published from `docs/` whenever the docs change on `release`.
- **Title bar island**: the companion orb and one line in the middle of the
  title bar say what the agent is doing in plain words, with one number
  (elapsed time, the turn's duration, or a retry countdown). The orb shows
  the same state as body language, orbited by running helpers, and reacts
  to you at the default **Lively** level. Click the island for the live
  steps, plan, helpers, cost and recent warnings; long notices, errors and
  notices with actions open it by themselves, and when the chat needs you,
  clicking it scrolls to the card. Beside it, an amber count of other chats
  that need you and a ring showing how full the context is (click it, or
  type `/context`, for the context card with **Compact now** and **Inspect
  prompt**). New setting `ui.companion` (`lively`, `calm`, `off`) under
  Settings → Appearance.
- **First-run setup** on a fresh install: connect a model (free OpenCode Zen
  models with no key, your own API key, or Ollama and LM Studio), add and
  trust a project, and choose the permission mode and companion. An
  animated splash shows while the app loads.
- **Project home**: the project and branch, a centred composer, starter
  prompts that fit the project, and cards for recent chats, uncommitted
  changes and project setup.
- **Inbox**: approvals (answerable in place), questions, plans and trust
  requests from every chat, chats that finished in the background, and
  every notice in full, kept for the app session. The sidebar badge counts
  what is waiting.
- Real window translucency: macOS vibrancy and Windows 11 Mica show through
  the window chrome; other systems get solid surfaces.

### Changed
- New visual system for the desktop app: design tokens with system colors
  and SF Pro, glass only for chrome that floats above content, a solid
  content sheet, spring and blur-in motion, and Reduce Motion and Reduce
  Transparency support.
- **Sidebar**: **New chat**, **Search** (the full command palette), **Home**
  and **Inbox** at the top; **Projects** (formerly Workspaces) with their
  branch, uncommitted-change count and a right-click menu, and one status
  dot and age per chat; the model in use (amber **Connect a model** when a
  key is missing), an **Update** button and Settings at the bottom. The
  title bar keeps only the changes button, with the number of files this
  chat changed.
- **Transcript and composer**: runs of tool calls fold into one line, small
  edits and successful commands stay folded, and subagents take one line.
  The turn receipt follows **Settings → Appearance → Task report detail**.
  Approval cards ask a question ("Allow Bash to run cargo test?") with
  **Allow once**, **Always allow…** (**In this chat** or **In this
  project**) and **Deny…**. The composer has a **+** menu, mode and model
  chips with reasoning effort inside the model picker, **Stop**, and an
  "Enter queues · ⌘Enter interrupts" hint; `!` output opens in a **Shell**
  drawer above it.
- **Changes panel** (formerly Review): **This chat** or **Uncommitted**,
  **Unified** or **Split**, a whole-window view, a compact file tree, file
  actions (open, reveal, copy the path or diff), syntax colors and folded
  unchanged lines.
- **Agents panel**: worktree changes **Ready to apply** come first, then
  running helpers with what each is doing now; finished helpers and usage
  fold away. Jobs show their live output and **Stop**. **New chat in a
  worktree** is a short dialog.
- **Prompt inspector** (formerly Context & Memory): a map of what fills the
  context window, an outline by kind with search, **Reader** and **Raw**
  views, and **Insights**.
- **Settings and palette**: Settings pages are grouped as General, Agent,
  Integrations and System; search finds single settings; a **Saving to**
  menu replaces the scope bar; long pages fold; source badges show only
  when a file sets a value; Settings reopens on the last page. The command
  palette is grouped, lists single settings as you type, and ranks results
  better.

### Removed
- The top-bar update chip, **Agents & jobs** button and Working/Review
  label, the working pill under the transcript, the todo strip above the
  composer, the live verification badge and the separate toast capsule.
  The title-bar island replaces them.
- Toasts that only confirmed something already visible (chat deleted,
  workspace added or removed, worktree created), the command palette's
  starter prompts (they remain on the home screen), and the home screen's
  logo, badge, tagline and shortcut rows.
- The historical v1 design documents, the old roadmap, `docs/deviations.md`
  and the old design specs under `docs/superpowers/`; git history keeps
  them.

### Fixed
- Deleting a chat or removing a project asks you to confirm first.
- The Changes panel's `[` and `]` keys (also `k` and `j`) step through the
  files; the diff panel's tooltips promised them, but nothing handled them.
- **Settings → Appearance → Task report detail** now changes what finished
  turns show; it used to be saved but ignored.
- The Hooks tab keeps the matcher of `SessionStart` and `PreCompact` hooks
  when it saves them, and offers a matcher field for both.
- OpenCode Zen free models no longer fail with `FreeTierError` ("can only
  be used from within OpenCode"): requests use the gateway's required
  client fingerprint (OpenCode user-agent, session id shape, and
  `shell`/`read` tool declarations).
- `verification.auto_checks` accepts a custom check by the id you gave it
  (`e2e`) as well as by its full id (`custom:e2e`).
- A built-in language server can be turned off with `enabled = false`
  alone; it no longer needs a `command`.
- The model catalog refreshes in the background once the cached copy is a
  day old, so newly released models appear without deleting the cache.
- The WebFetch setting (`web.fetch_extract`) is described as what it does:
  the fast model answers the agent's question about the page.
- The **Trust this workspace** setting and the chat's trust notices name
  everything an untrusted project is held back from; the list used to stop
  at hooks, MCP servers and checks.

### Security
- An untrusted workspace can no longer loosen your settings: its permission
  mode, allow rules, extra directories, shell, provider, web and
  language-server settings are ignored until you trust it (its deny and ask
  rules still apply). Its custom agents cannot run in a looser permission
  mode than their caller, and its custom commands' `allowed-tools` grant
  nothing.

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
