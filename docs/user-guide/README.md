# Z Engine 2.0 user guide

Z Engine is a desktop app that works on your code with you. You describe a
task in plain language ("fix the failing login test", "explain how billing
works"), and an AI model reads files, runs commands, edits code and checks
its own work. Before it does anything that changes your machine, Z Engine
asks for your approval, unless you have told it that action is fine.

Z Engine connects to an AI model provider of your choice (OpenRouter,
Anthropic, OpenAI, OpenCode Zen, or a model running on your own computer).
The model decides what to do; Z Engine carries it out, enforces your
permission rules, records everything, and lets you undo changes.

## What Z Engine can do

- **Understand a codebase.** Ask questions; the agent searches and reads the
  code and answers with file references.
- **Change code safely.** Edits and commands pass through one permission
  gate. You approve them one by one, or write rules that approve them for you.
- **Show its evidence.** Every turn that changes files gets a badge:
  **Verified**, **Unverified** or **Failed**, based on the tests and builds
  that actually ran.
- **Undo.** Before each message you send, Z Engine takes a snapshot of your
  project files. You can rewind the code, the conversation, or both.
- **Delegate.** The agent can start helper agents (subagents) that search,
  plan, review or verify in parallel, optionally in a separate copy of your
  repository.
- **Fit your workflow.** Custom agents, slash commands, skills, hooks and
  `AGENTS.md` instruction files, including existing Claude Code `.claude/`
  folders and `CLAUDE.md` files.

## A short tour of the window

| Area | What it is |
|---|---|
| **Sidebar** (left) | Your workspaces (project folders) and their chats. **New chat** starts a conversation and **Search** finds one. A dot on a chat means it is working, needs you (amber), finished while you were away, or its last response failed; the open chat reports its status in the title bar instead. Hover a chat or workspace to delete or remove it. |
| **Top bar** | In the middle, a small glass orb (the **companion**) and a **status line** name the workspace and chat and say what the agent is doing, with plan progress, time and cost (for example "Running the parser tests · 2/4 · 42s · $0.08"). The orb's expression follows the work and turns amber when you're needed, and the line briefly shows how each turn ended. Click the line for the plan, context use, cost, agents and recent notices. The buttons are **Review changes** (⌘D / Ctrl+D) and **Settings** (⌘, / Ctrl+,); while the sidebar is hidden, **New chat** and search (⌘K / Ctrl+K) move here too. |
| **Transcript** (center) | Your messages, the agent's replies, a card for every tool it uses, approval cards, and a footer under each turn with the badge, time and cost. |
| **Composer** (bottom) | Where you type. Above it: queued messages. Below it: the permission mode, model, reasoning effort, and an image button. |
| **Side panels** (right) | The agents and jobs panel, the diff (review) panel, and the git worktree panel open here. |

On the empty home screen, starter cards fill the composer with example
tasks. The composer's placeholder lists what `@`, `/`, `!` and `#` do.

## Contents

1. [Getting started](01-getting-started.md) – install, connect a model
   provider, open a project, and run your first task.
2. [Everyday use](02-everyday-use.md) – asking, fixing, reviewing,
   committing, attaching files, steering, rewinding and exporting.
3. [Permissions and safety](03-permissions-and-safety.md) – permission
   modes, approval cards, allow/ask/deny rules, workspace trust and the
   sandbox.
4. [Agents](04-agents.md) – subagents, background agents, worktree
   isolation, and writing your own agents.
5. [Plan mode, questions and todos](05-plan-mode-questions-and-todos.md) –
   reviewing plans before any change, answering the agent's questions, and
   the todo list.
6. [Commands and skills](06-commands-and-skills.md) – every slash command,
   writing custom commands, MCP prompts and skills.
7. [Memory and context](07-memory-and-context.md) – `AGENTS.md` files,
   rules, the context window, compaction and prompt caching.
8. [Hooks](08-hooks.md) – run your own scripts on agent events.
9. [MCP and code intelligence](09-mcp-and-code-intelligence.md) – connect
   MCP tool servers and language servers.
10. [Verification](10-verification.md) – badges, checks and verification
    modes.
11. [Models, providers and cost](11-models-providers-and-cost.md) – API
    keys, model roles, reasoning effort, fallbacks and cost tracking.
12. [Settings reference](12-settings-reference.md) – every settings key,
    file and environment variable.
13. [Sessions and data](13-sessions-and-data.md) – where things are stored,
    checkpoints, v1 import and privacy.
14. [Troubleshooting](14-troubleshooting.md) – `/doctor`, the log file and
    fixes for common problems.

> **Note:** Z Engine 2.0 is a desktop app only. There is no separate
> command-line version.

See also: [Getting started](01-getting-started.md)
