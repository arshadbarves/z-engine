# Agents

The agent you chat with can start *subagents*: separate agent runs, each
with its own context, instructions, tools and model, that carry out one task
and report back. This page covers the built-in agents, how to ask for one,
background agents, worktree isolation, custom agents, and the limits that
apply.

## What subagents are for

A subagent starts with a fresh context and sees only the task it was given,
not your conversation. That makes subagents useful for:

- **broad research** that would otherwise fill the main conversation with
  search results (the subagent returns only its findings);
- **parallel work**: several subagents started at once run at the same time;
- **isolated changes** in a separate git worktree, applied only after review;
- **independent checks**: a review or a slow test suite in the background.

The main agent decides when to delegate. Its report comes back to the main
agent, not to you; the main agent relays what matters.

## Built-in agents

| Agent | Used for | Tools | Model | Mode |
|---|---|---|---|---|
| `general` | Self-contained multi-step work that needs both investigation and changes. | All | Same as the caller | Same as the caller |
| `explore` | Fast, read-only searching and understanding of the codebase. Returns findings with `file:line` references. | Read, Glob, Grep, LSP, Bash, WebFetch, WebSearch | Fast model | Plan (read-only) |
| `plan` | Designing an implementation: returns a step-by-step plan with files, verification and risks. | Read, Glob, Grep, LSP, Bash, WebFetch, WebSearch | Same as the caller | Plan (read-only) |
| `review` | Reviewing pending changes (git diff, or a named file, commit or branch) for bugs, regressions, security issues and missing tests. | Read, Glob, Grep, Bash, LSP | Review model | Plan (read-only) |
| `verify` | Running the project's checks and reporting pass/fail with evidence, without fixing anything. | Read, Glob, Grep, Bash, Verify | Fast model | Same as the caller |

"Fast model" and "Review model" are the roles set in **Settings → Models**
(both default to your main model); see
[Models, providers and cost](11-models-providers-and-cost.md#model-roles).

Subagents never get the tools that talk to you (`AskUserQuestion`,
`ExitPlanMode`) or `ApplyAgentChanges`.

## Asking for a specific agent

Type `@` in the composer and pick an agent, or type `@agent-<name>`:

```text
@agent-explore how does the retry logic in the HTTP client work? Very thorough.
```

The main agent is then told to hand this request to that agent. You can
also ask in plain words ("use a review agent to check my changes").
`/review` always uses the `review` agent.

## Watching agents

Every subagent appears as an **Agent** card in the transcript with its
type, task, status, model, tokens, cost and duration. Click **Open
transcript** to follow it.

The **Agents & jobs** button in the top bar opens a side panel. Its badge
counts running agents, running background jobs, and worktree changes
waiting for you.

- **Agents** tab: every subagent of this chat as a tree (nested agents are
  indented), with status, model, tokens, cost, tool calls and time. Click a
  row to read its transcript and todo list. A **Usage by agent** table
  shows input tokens, output tokens and cost per agent.
- **Jobs** tab: background shells and background agents, with their output
  and a **Kill** button while they run.

Approval cards from a subagent show up in the main transcript, labelled
with the agent's type and task.

## Background agents and resuming

The main agent can start a subagent in the background: it continues its own
work and is notified when the subagent finishes. Background agents appear
in the **Jobs** tab; **Kill** stops one, **Transcript** opens it.

The agent can also *resume* a finished subagent, continuing it with its
full earlier context instead of starting over. Each report ends with the
agent's id; you can ask for this in plain words ("ask the explore agent
from before to also check the tests").

**Esc** cancels the current turn and the foreground subagents it started.
Background agents and shells keep running; stop them from the Jobs tab.
Closing the app stops everything.

## Worktree isolation

With worktree isolation, a subagent works in its own copy of your
repository instead of your project folder, so its edits don't touch your
files until you decide.

- The worktree is created at `.z-engine/worktrees/<agent-id>` on a new
  branch `zengine/<agent-id>`, starting from your current commit.
- The agent may read your main project but may change files only inside
  its worktree.
- When it finishes, its changes are committed on that branch and
  summarized (files changed and a diffstat).

Use it by asking for it ("do this in an isolated worktree") or by setting
`isolation: worktree` in a custom agent (below).

### Apply or discard

In the **Agents** panel, a finished worktree agent shows its branch, file
count and diffstat with two buttons:

- **Apply** merges the agent's changes into your working folder with a
  three-way merge, so your own uncommitted changes are kept. If the changes
  conflict, *nothing* is changed, the row says so (state `conflicted`), and
  the worktree is kept; you can resolve the conflicting files and try
  again, or discard.
- **Discard** deletes the worktree and its branch.

A worktree agent that changed nothing is cleaned up automatically. The main
agent can also apply changes itself with the `ApplyAgentChanges` tool,
which asks for your approval like any other edit. In a folder that isn't a
git repository, the agent runs in the shared project folder and a notice
tells you why.

> **Note:** The command palette's **New task in git worktree…** is
> different: it creates a worktree (`.z-engine/worktrees/<name>`, branch
> `zengine/<name>`), adds it as its own workspace, and starts a new chat in
> it.

## Custom agents

A custom agent is a markdown file with a YAML header (frontmatter) followed
by the agent's instructions (its system prompt).

```markdown
---
name: test-writer
description: Writes focused unit tests for code the caller names. Use it after a feature is implemented.
tools: Read, Glob, Grep, Edit, Write, Bash, Verify
model: fast
permissionMode: acceptEdits
maxTurns: 40
---

You write unit tests. Read the code under test and the existing tests
first, follow their style, cover edge cases, and run the tests with Verify.
Report the files you added and the test results.
```

### Where agent files go

Later locations override earlier ones when names match:

1. `~/.claude/agents/` (read when Claude compatibility is on)
2. `~/.config/z-engine/agents/` (your user folder; on Windows
   `%APPDATA%\z-engine\agents\`)
3. `<project>/.claude/agents/`
4. `<project>/.z-engine/agents/`

A custom agent with the name of a built-in (for example `explore`)
replaces it. You can also create and edit agents in **Settings → Agents &
Commands → Agents** (saved to your user folder or the project's
`.z-engine/`). `.claude` files are read, never written; editing one there
saves a copy in Z Engine's folder that takes precedence.

### Frontmatter fields

| Field | Meaning |
|---|---|
| `name` | The agent's name. Defaults to the file name without `.md`. |
| `description` | **Required.** When to use the agent; the main agent reads this to choose. |
| `tools` | Allowed tools, as a list or a comma-separated string. Omit it or use `*` for all tools. Entries ending in `*` match by prefix (`mcp__github__*`). |
| `disallowedTools` | Tools to remove (also `disallowed_tools`). |
| `model` | `inherit` (default: the caller's model), `main`, `fast`, `review`, or a model id. |
| `permissionMode` | `default`, `acceptEdits`, `plan` or `bypass`. Default: the caller's mode. An agent from an [untrusted](03-permissions-and-safety.md#workspace-trust) project can't use a looser mode than its caller. |
| `isolation` | `shared` (default) or `worktree`. |
| `maxTurns` | Model turns this agent may take (also `max_turns`). Default: `agents.max_turns`. |
| `color` | A display color; stored with the definition. |

> **Note:** `tools` controls which tools the agent has, not what they may
> do. `tools: Bash(git log:*)` gives the agent the whole Bash tool; what it
> may run is still decided by your permission rules.

Files that don't parse are skipped, and a notification names the file and
the problem. Changes made in Settings apply to open chats at once; files you
edit by hand are picked up by new chats (see
[Troubleshooting](14-troubleshooting.md#i-edited-a-file-by-hand-and-nothing-changed)).

## Limits

| Setting | Default | Meaning |
|---|---|---|
| `agents.max_concurrent` | 6 | Subagents running at once. Top-level and background agents wait for a free slot; nested foreground agents share their caller's slot. Read when a chat opens. |
| `agents.max_depth` | 2 | How deep subagents may nest: with 2, the main agent's subagents may start their own subagents, but those may not. 0 disables subagents. |
| `agents.max_turns` | 200 | Model turns per agent run (main agent included) before it stops. |
| `agents.session_cost_cap_usd` | 0 (off) | Stops the chat once it has spent this much. |

Set them under **Settings → Advanced → Agent limits**.

See also: [Plan mode, questions and todos](05-plan-mode-questions-and-todos.md) · [Permissions and safety](03-permissions-and-safety.md) · [Commands and skills](06-commands-and-skills.md)
