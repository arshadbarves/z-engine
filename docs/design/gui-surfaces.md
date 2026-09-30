# Z Engine GUI — Surfaces

Status: **canonical** · the companion to the [UI guide](gui-ui-guide.md).
It says which component and which pure function own each surface of the
app: the transcript and composer, the one home of every fact, the side
panel and overlays, and the pet. Keep it true when a surface moves.

## 1. Chat surfaces

- **Transcript.** Turns from `lib/domain/sessionView` and `lib/domain/timeline`:
  user cards (Copy), streamed text with a thinking disclosure, tool calls and
  a one-line receipt. `workSection()` (`timeline/groups.ts`) gathers a
  turn's thinking and tool calls into one work block ahead of its answer;
  once the turn is done and made two or more calls, `WorkSummary` folds it
  into one line (`workLine()`: "Worked for 1m 12s · read 6 files · ran 2
  commands"), with failed calls, agents, questions, plans, errors and
  compaction still in view. Inside, two or more consecutive calls fold into
  one run (`groupTurnItems()`, `summarizeRun()`; `ToolGroup`); `Agent`,
  `AskUserQuestion` and `ExitPlanMode` (`SOLO`) never fold. Edit, Write and
  Bash cards open only on failure; inline diffs reuse `DiffRows`. A fenced
  block is a `CodeBlock` whose colors arrive once it nears the screen and
  its reply has finished streaming.
- **Turn actions.** `TurnActions`, once at the end of a turn beside
  `TurnFooter`: Copy the answer (`answerText()`), Open changes (the Changes
  tab) and `RewindMenu` (Rewind to before this prompt). Compaction dividers
  that end a turn sit after them (`splitTrailingCompactions()`).
- **Receipt.** `TurnFooter` follows `ui.task_report_view` via
  `receiptPlan()` (`lib/domain/receipt.ts`): `quiet` (default) shows only a
  verdict that says something, `compact` adds file chips and duration ·
  cost, `detailed` adds tokens, Not applicable and open checks.
- **Long chats.** `Transcript` renders a window of turns
  (`timeline/window.ts`): the newest 30, 20 more each time the top comes into
  view, and at once back to a turn you jump to; only the newest 6 render in
  full, the rest fill in through `whenVisible()` as they near the screen.
  `ChatTimeline` is the prompt rail: a dot per prompt (two prompts or more,
  at most 40 spread by `sampleEvenly()`); a click jumps there.
- **Compaction.** While `view.compacting` (from `compactionStarted` until
  its `compacted` marker, a new main-agent reply, the turn's end or idle)
  the transcript ends in a "Compacting context…" row. Each marker is a
  divider in `LocalCards`, "Context compacted · 180k → 24k · Summary", whose
  disclosure holds the summary.
- **Approvals and questions** wait in the composer, not the transcript
  (`planning/PendingInteractions` inside `Composer`): the oldest approval
  (focused) or else the oldest question takes the draft's place, the draft
  is kept, and "N more waiting after this" counts the rest. `ApprovalCard`
  is phrased by `approvalQuestion()` (`lib/domain/approvals.ts`): Allow
  once, Always allow… (a `Menu`: this chat or this project), Deny… with
  feedback; y / s / p / n while focused. A plan waiting for review is a
  "Plan ready · Review" row in the transcript (`PlanReady`), which opens the
  Plan tab. Every waiting card or button (and the trust banner) has
  `data-pending-card`, which the island's button scrolls to and focuses.
- **Composer.** One floating glass surface (`Composer` with
  `ComposerInput`, `ComposerBar`, `ComposerPlusMenu`, `ComposerDropZone`,
  `ComposerQueue`, `ComposerAttachments`); the mode and model chips are
  `Popover`s, the model one with an `EffortRow` for reasoning models. Send
  and Stop are one button whose glyph swaps (Stop while a turn runs and
  the draft is empty).

## 2. Calm by default, one home per fact

**Calm by default.** At rest a surface says one line; the detail waits
behind a `Disclosure` or a click. Never fold away failures or anything
waiting on the user: failed steps show their last lines and cards open,
error notices open the island card, outcome notes always show, and
approvals, questions, plans and the trust banner stay in view and counted.

**One home per fact.** A summary may link to its home, and the ⌘K palette
and shortcuts may repeat any action, but two views never show the same fact.

| Fact | Home |
|---|---|
| Live status: step, retry, result, passing notice | the island (`Island`: `IslandCapsule`, `IslandCard`) |
| How full the context is | the context satellite (`ContextBubble`, `ContextCard`) |
| Other chats that need you | the waiting satellite (`WaitingBubble`), then the Inbox |
| Notice history, turns that finished in the background | the Inbox (`components/inbox/`) |
| What changed | the side panel's Changes tab (`ChangesButton` counts this chat's files) |
| The plan and its progress | the side panel's Plan tab (`PlanView`) |
| Helpers and background jobs | the side panel's Agents tab (`WorkPanel`) |
| What the model was sent | the side panel's Context tab (`PromptInspector`) |

- The island's state is the pure `liveStatus()` in `lib/domain/liveStatus.ts`,
  highest priority first: this chat needs you, a passing notice, provider
  retry, work in progress, a turn that just ended, the away recap, idle.
  `islandMode()` (`lib/domain/island.ts`) picks its shape: `rest` (pet and
  title), `live` (a step, retry, notice or result with its clock),
  `alert` (this chat needs you, with one Answer / Review / Approve button
  from `islandAction()` that focuses the waiting card) or `expanded` (the
  card). `islandLine()` makes it one line and at most one number;
  `noticeOpensIsland()` decides when a notice opens the card. Add a state
  there, with a test, rather than a new pill or banner.
- **The island card** (`IslandCard`, in the island's own surface, not a
  popover) summarizes and links to the homes above: the notice and its
  actions, Plan (done of total and the current step), the latest steps,
  Agents, Context, Waiting, Cost (this turn and the chat) and the last
  warnings. It closes on a click outside or Esc, which it takes before the
  side panel does.
- **The pet** shows the same state as body language (section 4).
- Toasts (`pushToast`) show on the island and stay in the Inbox: push one
  only for what the user cannot see; confirm visible actions at the control.
- Sidebar rows carry at most one mark (`sidebarMark()`); the open chat shows
  none, as the island names it. Sidebar controls (New chat, Search,
  Settings) join the title bar only while the sidebar is hidden.

## 3. The side panel and overlays

The side panel (`sidepanel/SidePanel`, `SidePanelTabs`; `sidepanel.css`) is
one glass card beside the stage with a tab band: Changes, Plan, Agents,
Context (`panelTabs()` in `lib/domain/sidePanel.ts`; a hidden `browser` tab
is reserved). Its state is `ui.panel { tab, open, expanded, width }`:
`openPanel(tab, target?, scope?)` opens a tab (Changes at a file and
scope), `togglePanel(tab?)` backs ⌘D, the Changes button and the panel
toggle. It resizes from its edge handle between 340 and 1100 px, always
leaving the stage 400 px (`clampPanelWidth()`; the width is kept on this
machine), or expands over the stage. `panelNudge()`, followed by
`followPanelNudges()`, opens the Plan tab for a newly pending plan and marks
Agents with a dot for a new helper; switching chats is not news. Tab labels
hide when the panel is 420 px wide or less; the empty span in the tab band
is the pet's `panel` perch.

| Surface | Components | Logic (`lib/domain/` unless noted) |
|---|---|---|
| Changes tab | `overlays/DiffPanel` with `DiffHeader`, `DiffFileTree`, `DiffView`, `DiffRows`, `DiffCode`, `DiffFileBar`; under 640 px wide it drops the layout switch and slims the file list, under 560 px it stacks the list above the diff (`diff.css`) | `foldContext()`, `splitRows()` in `diffRows.ts`; `highlightLine()` in `lib/highlight.ts` |
| Plan tab | `planning/PlanView` (review, edit, approve); `PlanReady` is its row in the transcript | `panelPlan()` in `plans.ts` |
| Agents tab | `agents/WorkPanel`, `ApplyCard`, `AgentRow`, `JobRow`; `AgentSummary` (with `pet/PetSprite`) is the one helper layout for its rows and the transcript's Agent card | `agentSections()`, `agentActivity()` in `agentTree.ts` |
| Context tab | `overlays/PromptInspector` with `InspectorMap` (a ring of the context window whose legend filters the outline), `InspectorOutline`, `InspectorInsights`; `InspectorReader` while expanded | `outlineGroups()` in `inspectOutline.ts` |
| Settings (full-window page) | `settings/SettingsPage`, `SettingsNav`, `ScopeMenu`, `SettingsGroup` | `SETTINGS_SECTIONS`, `searchSettings()` in `settings/searchIndex.ts`; `foldGroups()` in `components/settings/folding.ts` |
| Palette | `overlays/CommandPalette` | `paletteActions()` in `lib/paletteActions.ts`; `rankPalette()` in `palette.ts` |
| Worktree dialog, shell drawer | `overlays/WorktreeDialog`, `overlays/ShellOverlay` (inside `Composer`) | `create_worktree`; `lib/shellStore.ts` |
| Home, Inbox, first run | `home/`, `inbox/`, `onboarding/` | `startersFor()`, `setupChecklist()`; `inboxSnapshot()`; `needsOnboarding()`, `onboardingPose()` |

## 4. The pet

- `pet/IslandPet` fills the island's slot; `pet/PetLayer` (in `AppShell`)
  draws it while it roams, click-through except the pet, below every
  popover. Spots register with `use:perch` (`lib/ui/perch.svelte.ts`):
  slots `island`, `hero` (home), `empty` (the Inbox's `EmptyState` art),
  `panel` (the side panel's tab band); edges `composer`, `sidebar` (footer).
  While the side panel is expanded the stage's perches (`STAGE_PERCHES`:
  hero, empty, composer) are out of reach. Only `petBehavior()` in
  `lib/domain/pet/behavior.ts` moves it: a new spot is a `PerchId` and
  `PERCH_SIZE` in `perches.ts` plus a rule and a test.
- Perches are measured in a frame callback after one appears, goes or
  resizes, on window resize and scroll, and on the live clock's tick (twice
  a second while a turn runs or the pet is lively). `petMotion.svelte.ts`
  runs a frame loop only while the pet hops, walks, turns, is thrown, drops
  or swings from your grip; breathing is a CSS loop on the HTML
  `.pet-breath` wrapper, so an idle pet costs no frames.
- `ui.companion`: lively reacts to the agent and the user and roams while
  `ui.pet.roam`; calm reacts only to the agent and stays in the island;
  off shows the island's dot. Reactions (`petUi.react`: a boop, a level-up
  that `PetLayer` plays once the island is not asking for you) go through
  `reactionPose()` for both the island's pet and the roaming one.
- It only echoes `liveStatus()` (the tone tints its aura, the look its
  body; `aria-hidden`): never the only carrier of a fact, never
  interrupting. Under Reduce Motion its springs jump, it neither walks nor
  blinks, and `petIdle` plays no strolls or tricks. Only
  `lib/runtime/pet.svelte.ts` loads and saves `pet.json`
  ([how the pet works](../how-it-works/features-desktop-screens.md#the-pet)).
