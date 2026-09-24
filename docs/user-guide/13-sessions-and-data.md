# Sessions and data

This page explains where Z Engine keeps your chats, settings, keys and
logs on each operating system, how code checkpoints and rewind work, how v1
chats are imported, and what is sent to your model provider.

## Where things live

Z Engine uses two folders: a **config folder** for settings, keys and your
own extensions, and a **data folder** for chats, checkpoints and logs.

| | Config folder | Data folder |
|---|---|---|
| macOS | `~/.config/z-engine` | `~/Library/Application Support/z-engine` |
| Linux | `~/.config/z-engine` | `~/.local/share/z-engine` (or `$XDG_DATA_HOME/z-engine`) |
| Windows | `%APPDATA%\z-engine` | `%APPDATA%\z-engine` |

`ZENGINE_CONFIG_DIR` and `ZENGINE_DATA_DIR` move them. **Settings → About &
Updates → Files & Storage** shows the actual paths with copy buttons.

**Config folder:**

| Path | Contents |
|---|---|
| `settings.toml` | Your user settings. |
| `auth.json` | Stored API keys (owner-only permissions on macOS/Linux). |
| `trust.json` | Trusted workspace folders. |
| `AGENTS.md` | Your instructions for every project. |
| `agents/`, `commands/`, `skills/`, `rules/`, `output-styles/` | Your extensions for every project. |
| `models.json` | Optional model catalog additions. |
| `config.toml` | A v1 settings file, if any (read for import only, never changed). |

**Data folder:**

| Path | Contents |
|---|---|
| `sessions/<chat id>/` | One folder per chat (see below). |
| `sessions/<id>.jsonl` | v1 chat files not imported yet. |
| `checkpoints/` | Shadow git repositories for code checkpoints, one per project. |
| `cache/models-dev.json` | The cached model catalog. |
| `workspaces.json` | The workspace folders listed in the sidebar. |
| `z-engine-gui.log` | The app log. |

**In each project:**

| Path | Contents |
|---|---|
| `AGENTS.md`, `AGENTS.local.md` (and `CLAUDE.md`, `CLAUDE.local.md`) | Instructions. |
| `.z-engine/settings.toml`, `.z-engine/settings.local.toml` | Project and personal settings. |
| `.z-engine/agents/`, `commands/`, `skills/`, `rules/`, `output-styles/` | Project extensions. |
| `.z-engine/worktrees/` | Worktrees of isolated agents and worktree tasks. |

### Inside a chat folder

| File | Contents |
|---|---|
| `log.jsonl` | Every event of the chat in order: messages, turns, approvals, questions, plans, todos, agents, checks, checkpoints, compactions, rewinds, mode/model changes, usage. |
| `meta.json` | Title, project, dates, message count, cost. Rebuilt from the log if missing. |
| `agents/<agent id>.jsonl` | Each subagent's transcript. |
| `artifacts/` | Full outputs that were too long for the chat: long command output, cleared tool results, check logs. |

Chats are written as they happen, so they survive a crash. If the app
closes in the middle of a turn, that turn shows **Interrupted · the app
closed mid-turn** when you reopen the chat; damaged lines at the end of a
log are skipped with a notice.

Deleting a chat (trash icon) removes its folder (and its v1 file). It does
not remove code checkpoints, which are shared by all chats of a project.

## Checkpoints and rewind

Before each message you send, Z Engine snapshots your project's files into
a *shadow* git repository in the data folder's `checkpoints/`. It never
touches your own repository, index or configuration, and it captures
changes made by shell commands too, not only by the agent's edit tools.
The agent's first tool calls in a turn wait until the snapshot is taken.

What a checkpoint leaves out:

- `.git` folders and `.z-engine/worktrees/`;
- everything your repository ignores (`.gitignore` and similar);
- files larger than 8 MiB.

Checkpoints need git installed. For projects with more than 50,000 files
(after ignores), checkpoints are turned off and a notice says so;
`/status` shows whether they're on.

**Rewinding** (hover a message → **Rewind**) offers *Code and
conversation*, *Conversation only* or *Code only*
([Everyday use](02-everyday-use.md#rewind)). Restoring code:

1. takes a snapshot of the current state first;
2. rewrites only the files that differ: changed or deleted files come back,
   files created since are deleted (with folders they leave empty);
3. leaves alone any file that a snapshot couldn't store (ignored or too
   large at either time), and lists it in the notice.

Rewinding the conversation records the rewind in the log and drops later
messages from what you and the model see. The agent forgets which files it
had read, so it reads them again before editing.

The diff panel's **Chat** scope compares the project with the chat's first
checkpoint.

> **Warning:** Rewind can't undo effects outside your project files:
> installed packages, database changes, network calls, `git push`.

## Importing v1 chats

v1 kept each chat as a single `<id>.jsonl` file in the sessions folder.
Z Engine 2.0 lists those chats in the sidebar with a **v1** tag. The first
time you open one, it is converted into a chat folder with the same id; the
original file is left untouched, and opening it again doesn't import it
twice. v1 settings are covered in
[Settings reference](12-settings-reference.md#importing-v1-settings).

## Privacy: what leaves your computer

**Sent to your model provider** with each request:

- the system prompt, which includes your instruction files and rules, the
  repository map (names of functions, types and files), the project folder
  path, platform, shell, date, and a git snapshot (branch, `git status`
  output, recent commit messages);
- the tool definitions, including any MCP tools;
- the conversation: your messages, attached files and images, the agent's
  replies, and the results of its tool calls, such as the contents of files
  it read and command output.

The fast model also receives your first message (to write a title), older
history (for compaction summaries), and web pages the agent fetched (to
answer its question about them).

**Sent elsewhere:**

- Web searches go to your search provider (Brave, Tavily, Exa or your
  SearXNG), and WebFetch requests go to the sites being fetched.
- MCP servers receive the arguments of the tool calls made to them.
- Z Engine downloads the model catalog from `models.dev` when it has none
  cached, and checks GitHub for updates when it starts.

**Stays on your computer:** your chats, logs and artifacts, checkpoints,
settings, keys (except each key going to its own provider), and trust
decisions. Z Engine has no account and sends no usage data of its own.

> **Tip:** To keep secrets away from the model, deny them:
> `deny = ["Read(./.env)", "Read(~/.ssh/**)"]`. A deny rule for `Read`
> also stops shell commands from reading those files.

See also: [Everyday use](02-everyday-use.md) · [Settings reference](12-settings-reference.md) · [Troubleshooting](14-troubleshooting.md)
