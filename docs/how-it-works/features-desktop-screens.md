# The desktop app's screens

What each main surface of the window does, from the project home to the
palette. How the window talks to the engine, how events become the screen and
how the app updates itself are on [The desktop app](features-desktop-app.md).
Terms are in the [glossary](glossary.md); the UI rules in the
[GUI UI guide](../design/gui-ui-guide.md).

## The project home, the Inbox and first run

**In plain words.** With no chat open, a project opens on its home page, a
desk with today's list on it. The Inbox is the tray where whatever happened
while you looked elsewhere waits.

**How it works**
- The home names the project and branch, asks "What should we work on?"
  and offers starter prompts that fit the project, then cards shown only
  when they apply: **Continue**, **Uncommitted changes** and **Set up this
  project**. The first message turns the home into the chat.
- The **Inbox** lists what **needs you** in every chat (approvals can be
  answered there), chats that **finished while you were away**, and every
  **notice** in full while the app runs; its badge counts all three.
- A fresh install (no projects, no chats) first sets up a model, a project
  and its trust, the permission mode and the companion.

**For developers**
- `stageFor()` in [`lib/domain/stage.ts`](../../crates/z-engine-gui/ui/src/lib/domain/stage.ts)
  picks home, chat or inbox for [`chrome/MainStage.svelte`](../../crates/z-engine-gui/ui/src/components/chrome/MainStage.svelte);
  [`components/home/`](../../crates/z-engine-gui/ui/src/components/home/),
  [`inbox/`](../../crates/z-engine-gui/ui/src/components/inbox/) (over
  `inboxSnapshot()`; `pushToast` records the last 200 notices) and
  [`onboarding/`](../../crates/z-engine-gui/ui/src/components/onboarding/)
  (when `needsOnboarding()`) draw the rest.

## The transcript and tool cards

**In plain words.** The transcript is the chat history drawn as cards.
Routine tool work folds into one line, like a receipt you can unfold for
the details; failures stay in view.

**How it works**
- A turn shows your message, the reply (Markdown, with a collapsible
  thinking section), its tool calls and a one-line receipt. Two or more
  tool calls in a row fold into one line ("Read 2 files · ran 1 command")
  with the step in progress and how many failed or were denied; `Agent`,
  `AskUserQuestion` and `ExitPlanMode` stand alone. A failure shows its
  last three lines, a running command three live lines.
- A card follows `toolStarted`, `toolProgress` and `toolFinished` (ok,
  error, denied or cancelled) and depends on the tool's *family*: a diff
  for edits, command and output for Bash (both open by themselves only on
  failure), one line for Agent, own cards for TodoWrite and the web tools,
  a generic card for the rest, MCP tools included.
- The receipt follows `ui.task_report_view`: **quiet** (default) shows only
  a verdict that says something; **compact** adds the changed files (each
  opens the Changes panel there), duration and cost; **detailed** adds
  tokens, Not applicable and the checks. A turn that did not complete
  always says so.

**For developers**
- [`components/chat/`](../../crates/z-engine-gui/ui/src/components/chat/):
  runs from `lib/domain/timeline/groups.ts`, receipts from
  `lib/domain/receipt.ts`; [`chat/tools/ToolCall.svelte`](../../crates/z-engine-gui/ui/src/components/chat/tools/ToolCall.svelte)
  picks the card from the family in [`lib/domain/tools/toolMeta.ts`](../../crates/z-engine-gui/ui/src/lib/domain/tools/toolMeta.ts)
  (a new card: a `FAMILIES` entry, a component, a branch in `ToolCall`).

## Approval, question and plan cards

**In plain words.** When the agent needs your say (permission, an answer, or
approval of a plan), it pauses and puts a card in the chat, like a builder
who knocks before taking down a wall.

**How it works**
1. The engine's *gate* decides a tool call must ask (see
   [core features](features-core.md)) and emits `approvalRequested` with a
   title and a preview, such as the diff of an edit.
2. The card asks a question ("Allow Bash to run cargo test?") with the
   reason and a preview folded after six lines: **Allow once**, **Always
   allow…** (**In this chat**, a session rule, or **In this project**, also
   saved to `.z-engine/settings.local.toml`), or **Deny…** with feedback for
   the model; y, s, p and n answer a focused card (`resolveApproval`).
3. `AskUserQuestion` and `ExitPlanMode` show a question card
   (`answerQuestion`) and a plan card (`resolvePlan`); see
   [plan mode, questions and todos](features-interaction.md#plan-mode-structured-questions-and-todos).
   An untrusted project that sets anything trust withholds gets a trust
   banner (`trustRequired`, answered with `trustWorkspace`).
4. While a card waits you can still steer, change mode or cancel. Cards from
   a subagent carry that agent's label.

**For developers**
- [`components/planning/PendingInteractions.svelte`](../../crates/z-engine-gui/ui/src/components/planning/PendingInteractions.svelte)
  renders `chat/ApprovalCard` (worded by `lib/domain/approvals.ts`),
  `planning/QuestionCard`, `planning/PlanCard` and `chat/TrustBanner`, each
  marked `data-pending-card`. Types: `ApprovalRequest`, `ApprovalDecision`,
  `PlanDecision`, `Question`, `QuestionAnswer`; the engine's
  [`broker/`](../../crates/z-engine-engine/src/broker/) holds pending
  requests so the session actor keeps accepting commands.

## The island and the companion

**In plain words.** The middle of the title bar is a small pill, the
*island*: a glass orb with eyes and one line about what the agent is doing,
like a colleague you can see across the desk. A ring beside it shows how
full the context is; an amber count, how many other chats need you.

**How it works**
- The line shows one state, most urgent first: this chat needs you, a
  passing notice, a provider retry, work in progress (the step in plain
  words), a turn that just ended, a recap of turns that ended while you
  were away, or idle (the chat's title), with at most one number.
- Clicking it opens the island sheet: the full message and its actions, the
  latest steps, the plan, running helpers, cost, and recent warnings; a
  long, failing or actionable notice opens it by itself. While the chat
  needs you, clicking scrolls to the waiting card instead.
- The ring shows a percentage from 65% (amber; red from 85%); it and
  `/context` open the context card: use by prompt layer, **Compact now** and
  **Inspect prompt**. The amber count opens the waiting chat, or the Inbox.
- The orb's pose follows the same state: its eyes scan while the agent
  reads, it bobs while commands run, looks at you in amber when it needs
  you, hops when a turn is verified and droops when one fails. Running
  agents orbit it; a ring shows the `TodoWrite` plan's progress.
- At the default **Lively** level (`ui.companion`) it also reacts to you
  (typing, scrolling back, quiet minutes, coming back); **Calm** reacts only
  to the agent; **Off** shows a dot. Under Reduce Motion it holds still.

**For developers**
- [`chrome/TitleStatus.svelte`](../../crates/z-engine-gui/ui/src/components/chrome/TitleStatus.svelte)
  holds `WaitingBubble`, `Island` (with `IslandSheet`) and `ContextBubble`
  (with `ContextCard`); `liveStatus()` in [`lib/domain/liveStatus.ts`](../../crates/z-engine-gui/ui/src/lib/domain/liveStatus.ts)
  picks the state, `lib/domain/island.ts` shapes the pill, and
  `companionPose()` in `lib/domain/companion.ts` the orb (`Companion.svelte`).
- The card calls `context_breakdown` ([`Engine::context_breakdown`](../../crates/z-engine-engine/src/engine/queries/context.rs)),
  the estimate `/context` prints, which also arrives as `contextReport`.

## The agents and jobs panel

**In plain words.** Helpers the agent sends off (*subagents*) and commands
left running in the background (*jobs*) are listed in one panel, like a
board showing who is out on an errand.

**How it works**
- A subagent emits `agentStarted`, then `agentUpdated`; a background shell
  or agent emits `jobUpdated`. Open the panel from the island sheet,
  `/agents`, `/jobs` or the palette.
- Agents come in the order they need you: **Ready to apply** (a worktree
  agent's branch and diffstat, with **Apply** and **Discard**), **Working**
  with a live line of what each does, then **Finished** and usage, folded.
  Opening one loads its transcript (`agent_transcript`). A running job
  shows its last six lines and **Stop**.

**For developers**
- [`components/agents/`](../../crates/z-engine-gui/ui/src/components/agents/)
  (`WorkPanel`, `ApplyCard`, `AgentRow`, `JobRow`, `AgentTranscript`),
  `agentSections` and `agentActivity` in `lib/domain/agentTree.ts`;
  commands `applyAgentChanges`, `discardAgentChanges`, `killJob`. How
  agents run: [agents and context](features-agents-and-context.md).

## The Changes panel

**In plain words.** The Changes panel shows what changed in your files, like
"track changes" in a word processor: either what this chat changed, or
everything not yet committed.

**How it works**
- **This chat** compares the project with the chat's first *checkpoint*, a
  copy of your files kept in a hidden git repository, without taking a new
  one. **Uncommitted** compares the working tree with your last commit
  (`HEAD`), with line counts. Both need git.
- It docks beside the chat or fills the window, Unified or Split, with
  syntax colors and long unchanged stretches folded; a file can be opened
  in its default app, shown in its folder, or copied. Open it with
  the title bar's Changes button (a count of this chat's files), ⌘D /
  Ctrl+D or a receipt's file; `[` and `]` step through files.

**For developers**
- [`components/overlays/DiffPanel.svelte`](../../crates/z-engine-gui/ui/src/components/overlays/DiffPanel.svelte)
  and its `Diff*` parts, rows from `lib/domain/diffRows.ts`. This chat:
  `session_changed_files`, `session_diff_for_file` ([`engine/queries/changes.rs`](../../crates/z-engine-engine/src/engine/queries/changes.rs)).
  Uncommitted: `list_changed_files`, `diff_for_file` ([`engine/queries/git.rs`](../../crates/z-engine-engine/src/engine/queries/git.rs),
  which also creates the worktree for **New chat in a worktree…**); they
  take a project root, else the active project, and refuse other folders.
- `open_path` and `reveal_path` ([`commands/workspace.rs`](../../crates/z-engine-gui/src-tauri/src/commands/workspace.rs))
  use the opener plugin once `guard::project_path` finds the path inside a
  known project, symlinks resolved.

## The prompt inspector

**In plain words.** The prompt inspector lets you look over the agent's
shoulder at exactly what was last sent to the model, like reading a letter
before it goes in the post.

**How it works**
- It fetches the last request sent to the provider (`inspect_request`) and
  lays it out: a map of how much of the context window each kind of part
  fills (instructions, project, conversation, tools), an outline to search,
  and a reader for one part, rendered or raw. **Insights** names the
  largest part and what repeats across turns; **Copy all** copies it all.
  Open it with **Inspect prompt** in the context card or the palette.

**For developers**
- [`overlays/PromptInspector.svelte`](../../crates/z-engine-gui/ui/src/components/overlays/PromptInspector.svelte)
  with its `Inspector*` parts; parsing in `lib/domain/requestInspect.ts`,
  the outline in `lib/domain/inspectOutline.ts`; data from `Engine::last_request`.

## Settings and scopes

**In plain words.** Settings are stacked like transparent sheets: defaults,
then yours, then the project's, then your private project tweaks. The
settings screen picks the sheet you write on and shows where each value
comes from.

**How it works**
- Pages (⌘, or Ctrl+,) are grouped: **General** (Models, Providers,
  Appearance), **Agent** (Permissions, Memory, Verification, Agents &
  Commands), **Integrations** (MCP, Hooks) and **System** (Advanced, About
  & Updates). Settings reopens where you left it; search finds single
  settings by everyday words and scrolls to them.
- A layered page names the file it saves to (**Saving to**): **User** (every
  project), **This project** (`.z-engine/settings.toml`, shared) or
  **Personal (local)** (`.z-engine/settings.local.toml`, kept out of git).
  A badge names where a value came from; defaults have none. Saving writes
  that one file and reloads every open session for a user change, else
  that project's sessions.
- Agents, commands, skills, rules and `AGENTS.md` are edited as files, after
  a check that the path is one the app knows.

**For developers**
- [`components/settings/`](../../crates/z-engine-gui/ui/src/components/settings/):
  `SettingsPage.svelte` switches on the page ids of `SettingsNav.svelte`,
  grouped and searched through `lib/domain/settings/searchIndex.ts`; one
  `*Tab.svelte` per page; form logic in `lib/domain/settings/`.
- Shell: [`commands/settings.rs`](../../crates/z-engine-gui/src-tauri/src/commands/settings.rs)
  (`set_setting`, `add_permission_rule`, `set_hooks`, ...), `layers.rs`,
  `commands/access.rs` (keys, trust), `commands/extensions.rs` and `guard.rs`.

## The composer, palette and shortcuts

**In plain words.** The composer is the message box; the command palette is
a search box for the whole app: press ⌘K, type a few letters, and jump to a
chat, a project, a setting or an action.

**How it works**
- The composer sends `submit`. While a turn runs, a new message is queued as
  steering (pills you can edit, sent as `editQueue`); ⌘Enter / Ctrl+Enter
  sends `interrupt`, and Esc or the stop button `cancel`. Its **+** menu
  attaches images and inserts `@` (mentions), `/` (commands), `#` (a memory
  note) and `!` (your own `shell` command, output in a drawer above it).
- The palette (⌘K / Ctrl+K, or **Search** in the sidebar) lists actions,
  places, recent chats, this chat's tools, modes and projects; typing also
  finds single settings.
- Global shortcuts (⌘ on macOS, Ctrl elsewhere): K palette, N new chat,
  B sidebar, D Changes panel, comma settings. In the composer, Shift+Enter
  adds a line, Shift+Tab cycles the permission mode, ↑ and ↓ walk your history.

**For developers**
- [`chat/Composer.svelte`](../../crates/z-engine-gui/ui/src/components/chat/Composer.svelte)
  and its `Composer*` parts; keys in [`lib/domain/composerKeys.ts`](../../crates/z-engine-gui/ui/src/lib/domain/composerKeys.ts);
  app-only slash commands in `lib/stores/uiCommands.ts`.
- [`overlays/CommandPalette.svelte`](../../crates/z-engine-gui/ui/src/components/overlays/CommandPalette.svelte)
  with `lib/paletteActions.ts` and `lib/domain/palette.ts`; global keys in
  `lib/stores/shortcuts.ts`.

See also: [The desktop app](features-desktop-app.md) · [How Z Engine works](README.md) ·
[Everyday use](../user-guide/02-everyday-use.md) · [Agents](../user-guide/04-agents.md) ·
[Settings reference](../user-guide/12-settings-reference.md) · [The crates](crates.md)
