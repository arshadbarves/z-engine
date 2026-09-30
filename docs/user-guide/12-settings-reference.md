# Settings reference

Z Engine's settings are TOML files. This page explains the files and how
they combine, how the Settings screens map to them, every key with its type
and default, the environment variables Z Engine reads, and how v1 settings
are imported.

## Settings files and layers

Settings are read in layers, lowest precedence first. A later layer wins.

| Layer | File | Use it for |
|---|---|---|
| Defaults | built in | – |
| **User** | `~/.config/z-engine/settings.toml` (Windows: `%APPDATA%\z-engine\settings.toml`) | Your preferences for every project. |
| **Project** | `<project>/.z-engine/settings.toml` | Settings shared with your team through git. |
| **Personal (local)** | `<project>/.z-engine/settings.local.toml` | Your own overrides for this project. Z Engine adds it to `.z-engine/.gitignore` when it writes the file. |
| **Environment** | `ZENGINE_MODEL`, `ZENGINE_BASE_URL`, `ZENGINE_PROVIDER`, `ZENGINE_SHELL` | Overriding everything, for example in scripts. |

How values combine:

- Single values (strings, numbers, true/false) are replaced by the later
  layer. Tables such as `[provider.headers]` and `[shell.env]` merge key by
  key.
- These lists **combine** across layers (duplicates removed):
  `permissions.allow`, `permissions.ask`, `permissions.deny`,
  `permissions.additional_directories`, `shell.env_passthrough`,
  `shell.sandbox.extra_writable`.
- Hook lists (`[[hooks.<Event>]]`) are **concatenated**: all run.
- `[[verification.checks]]` merge by `id`: a later check with the same id
  replaces the earlier one.
- `[mcp.servers.<name>]` and `[lsp.servers.<name>]`: a later server with the
  same name replaces the earlier one completely.
- Other lists (such as `model.fallbacks` and `verification.auto_checks`)
  are replaced by the later layer.

Problems never stop the app:

- Out-of-range numbers are clamped and reported.
- Unknown keys (typos) are reported and ignored.
- A file with a wrong value type is **skipped entirely** until you fix it;
  while that file is selected, the Settings page says "This settings file
  is not applied until it is fixed" and why.
- Files larger than 1 MiB are refused.
- In an untrusted project, anything that runs programs or loosens
  permissions comes from your user settings only: `hooks`, `mcp`,
  `verification.checks`, `permissions.mode`, `permissions.allow`,
  `permissions.additional_directories`,
  `permissions.auto_allow_read_only_bash`, and the whole `shell`,
  `provider`, `web` and `lsp` sections. The project's `deny` and `ask`
  rules still apply ([workspace trust](03-permissions-and-safety.md#workspace-trust)).

The first line of each file is `schema = 2`; the app manages it.

## Settings screens and files

Open Settings with ⌘, (Ctrl+, on Windows and Linux) or the gear at the
bottom of the sidebar. It opens on the page you used last; links elsewhere
in the app, such as **Connect a model** on the project home, open a given
page.
**Back** or Esc closes it. The navigation groups the pages, and each page
starts with one line saying what it is for:

| Group | Page | Keys |
|---|---|---|
| General | Models | `model.*` |
| | Providers | `provider.*`, `model.main`; keys go to `auth.json` |
| | Appearance | `ui.task_report_view`, `ui.output_style` |
| | Pet | `ui.companion`, `ui.pet.*` |
| Agent | Permissions | `permissions.*` |
| | Memory | `AGENTS.md` files |
| | Verification | `verification.*` |
| | Agents & Commands | files in `agents/`, `commands/`, `skills/`, `rules/`, `output-styles/` (user folder or project `.z-engine/`) |
| Integrations | MCP | `mcp.servers.*` |
| | Hooks | `hooks.*` |
| System | Advanced | `context.*`, `agents.*`, `web.*`, `shell.*`, `lsp.*`, `compat.claude`, workspace trust |
| | About & Updates | version, updates, file locations |

- **Search settings** at the top of the navigation finds single settings
  by name or by everyday words: "budget" finds **Session cost cap**, "api
  key" finds **Providers and API keys**. Pick a result with ↑/↓ and Enter,
  or click it: its page opens, its group unfolds, and the row is briefly
  highlighted. The command palette finds the same settings.
- Pages that edit settings files show **Saving to** at the top right. Its
  menu offers **User**, **This project** and **Personal (local)**, each with
  a short description, and shows the path of the file it writes. Changes
  apply to open chats immediately. Without an open project only **User** is
  available, and the page says "Open a project to edit its project and
  personal settings."
- Some groups fold to their title and one line until you open them: every
  group on **Advanced** (Context, Agent limits, Web, Shell, Language
  servers, Workspace), **Fallbacks** and **Requests** on **Models**, and
  **Folders** on **Permissions**.
- A setting shows a source badge (**User**, **Project**, **Personal** or
  **Environment**) only when a settings file or environment variable sets
  it; hover the badge for the file. **Reset** removes the value from the
  selected file, and a note says when a higher file overrides it.

> **Warning:** When the app saves a settings file it rewrites it, so
> comments you added by hand are lost.

## Model

`[model]` – which models run. See [Models, providers and cost](11-models-providers-and-cost.md).

| Key | Type | Default | Meaning |
|---|---|---|---|
| `main` | string | `"anthropic/claude-sonnet-4.5"` | Model of the main agent. Empty uses the default. |
| `fast` | string | unset (= main) | Titles, summaries, WebFetch answers, `explore`/`verify` agents. |
| `review` | string | unset (= main) | The `review` agent. |
| `fallbacks` | list of strings | `[]` | Tried in order after a temporary failure. |
| `effort` | `"low"`, `"medium"`, `"high"`, `"max"` | unset | Reasoning effort for new chats. Unset sends none. |
| `max_output_tokens` | integer | `16384` | Output limit per request, 256–200000. |
| `context_window` | integer | unset (catalog) | Overrides the model's context size. |

`[pricing."<model id>"]` – your own prices in US dollars per million
tokens: `input`, `output` (required), `cache_read`, `cache_write`
(optional; default to `input`). The id matches with or without a `vendor/`
prefix.

## Provider

`[provider]` – how the model API is reached.

| Key | Type | Default | Meaning |
|---|---|---|---|
| `kind` | `"auto"`, `"openai_chat"`, `"anthropic"` | `"auto"` | Wire format. `auto` uses Anthropic for `anthropic.com` hosts, OpenAI-compatible otherwise. |
| `base_url` | string | `"https://openrouter.ai/api/v1"` | API address; trailing `/` is removed. |
| `headers` | table | `{}` | Extra HTTP headers for every model request. |
| `cache_control` | bool | unset | Prompt caching. Unset: on for Anthropic and OpenRouter, off elsewhere. |

## Permissions

`[permissions]` – see [Permissions and safety](03-permissions-and-safety.md).

| Key | Type | Default | Meaning |
|---|---|---|---|
| `mode` | `"default"`, `"acceptEdits"`, `"plan"`, `"bypass"` | `"default"` | Mode new chats start in. Also accepts `bypassPermissions`, `readOnly`, `accept_edits`. |
| `allow` | list of rules | `[]` | Run without asking. |
| `ask` | list of rules | `[]` | Always ask. |
| `deny` | list of rules | `[]` | Never allow, in any mode. |
| `additional_directories` | list of paths | `[]` | Extra folders the agent may read and edit. `~` and project-relative paths allowed. |
| `auto_allow_read_only_bash` | bool | `true` | Run recognised read-only commands without asking. |

## Context

`[context]` – see [Memory and context](07-memory-and-context.md).

| Key | Type | Default | Meaning |
|---|---|---|---|
| `compact_at_percent` | integer | `92` | Summarize older history at this context fill, 50–99. |
| `keep_recent_tool_results` | integer | `8` | Tool results kept word for word when older ones are cleared. |
| `repo_map` | bool | `true` | Include the repository map. |
| `repo_map_chars` | integer | `6000` | Size of the repository map. |

## Agents

`[agents]` – see [Agents](04-agents.md#limits).

| Key | Type | Default | Meaning |
|---|---|---|---|
| `max_concurrent` | integer | `6` | Subagents at once (at least 1). Applies to chats opened afterwards. |
| `max_depth` | integer | `2` | Subagent nesting depth; 0 disables subagents. |
| `max_turns` | integer | `200` | Model turns per agent run (at least 1). |
| `session_cost_cap_usd` | number | `0.0` | Stop a chat at this spend; 0 turns it off. |

## Verification

`[verification]` – see [Verification](10-verification.md).

| Key | Type | Default | Meaning |
|---|---|---|---|
| `mode` | `"off"`, `"report"`, `"auto"`, `"strict"` | `"report"` | What happens at the end of a turn that changed files. |
| `max_continuations` | integer | `3` | Times auto/strict may send a turn back, 0–10. |
| `auto_checks` | list | `["test"]` | Check kinds or check ids run by auto/strict. A custom check can be listed as `e2e` or `custom:e2e`. |

`[[verification.checks]]` – one table per check: `id` (required),
`command` (required), `label` (default: the id), `kind` (`test`, `build`,
`lint`, `typecheck`, `format`, `custom`; default `custom`), `cwd` (relative
to the project), `timeout_secs` (default 600, at least 1).

## Hooks

`[[hooks.<Event>]]` – events: `SessionStart`, `UserPromptSubmit`,
`PreToolUse`, `PostToolUse`, `Stop`, `SubagentStop`, `PreCompact`,
`Notification`, `SessionEnd`. Fields: `command` (required), `matcher`
(regular expression, optional), `timeout_secs` (default 60, at least 1).
See [Hooks](08-hooks.md).

## MCP

`[mcp.servers.<name>]` – fields: `command`, `args`, `env`, `cwd` (stdio) or
`url`, `headers` (HTTP); `enabled` (default `true`), `disabled_tools`
(default `[]`), `timeout_secs` (default 60). Exactly one of `command` and
`url`. See [MCP and code intelligence](09-mcp-and-code-intelligence.md).

## LSP

| Key | Type | Default | Meaning |
|---|---|---|---|
| `lsp.enabled` | bool | `true` | Use language servers. |

`[lsp.servers.<name>]` – fields: `command` (required unless the server is
turned off), `args`, `extensions`, `root_markers`, `enabled` (default
`true`; `false` also turns off a built-in server of that name).

## Web

`[web]` – how the agent searches and reads the web.

| Key | Type | Default | Meaning |
|---|---|---|---|
| `search_backend` | `"none"`, `"brave"`, `"tavily"`, `"exa"`, `"searxng"` | `"none"` | Web search provider. With `none` the WebSearch tool isn't offered. Brave, Tavily and Exa need a key (stored from Settings → Advanced → Web, or `BRAVE_API_KEY` / `TAVILY_API_KEY` / `EXA_API_KEY`); without one, search stays off. |
| `search_url` | string | unset | Base URL of your SearXNG instance (required for `searxng`). |
| `fetch_extract` | bool | `true` | WebFetch hands the page to the fast model, which answers the agent's question about it. `false` returns the page itself (as markdown). |
| `allow_private_network` | bool | `false` | Let WebFetch reach localhost and private network addresses. |

## Shell

`[shell]` – how commands run.

| Key | Type | Default | Meaning |
|---|---|---|---|
| `path` | string | unset (detect) | The shell. macOS/Linux: bash, zsh, sh, dash or ksh (default: your login shell if bash or zsh, else `/bin/bash`, then `/bin/sh`). Windows: `bash` (Git Bash), `powershell`/`pwsh`, `cmd`, or a path (default: Git Bash, then PowerShell, then cmd). |
| `env_passthrough` | list of names | `[]` | Extra variables copied from Z Engine's environment into commands, for example `SSH_AUTH_SOCK`. |
| `env` | table | `{}` | Variables set for every command; they override everything else. |

Commands (Bash, background shells, checks, hooks) don't get your whole
environment. They get `PATH`, `HOME`, `SHELL`, `TERM`, `LANG`, `LC_ALL`,
`TMPDIR`, `USER`, `LOGNAME` (plus Windows system variables such as
`USERPROFILE`, `APPDATA` and `Path`), the `env_passthrough` variables, and
`GIT_TERMINAL_PROMPT=0`, `PAGER=cat`, `GIT_PAGER=cat`, `ZENGINE=1`, then
`env`.

`[shell.sandbox]` – see [the sandbox](03-permissions-and-safety.md#the-sandbox).

| Key | Type | Default | Meaning |
|---|---|---|---|
| `enabled` | bool | `false` | Run commands in the OS sandbox. |
| `allow_network` | bool | `false` | `false` blocks all connections except localhost. |
| `extra_writable` | list of paths | `[]` | More writable folders (`~/` = home, relative = project). |
| `auto_allow` | bool | `true` | Run sandboxed commands whose writes stay inside without asking. |

## UI and compatibility

| Key | Type | Default | Meaning |
|---|---|---|---|
| `ui.companion` | `"lively"`, `"calm"`, `"off"` | `"lively"` | How lively the [pet](02-everyday-use.md#your-pet) is: **Lively** reacts to the agent and to you (typing, scrolling, stepping away) and roams if `ui.pet.roam` is on, **Calm** reacts only to the agent and stays in the title bar, **Off** shows a small dot instead of the pet. Settings → Pet → Liveliness. |
| `ui.pet.name` | string | `"Zen"` | The pet's name. Trimmed; empty means `"Zen"`; longer than 24 characters is cut with a warning. Settings → Pet → Name. |
| `ui.pet.look` | `"pearl"`, `"mint"`, `"sky"`, `"lilac"`, `"peach"`, `"graphite"` | `"pearl"` | The pet's body color. Settings → Pet → Look. |
| `ui.pet.roam` | bool | `true` | At the **Lively** level, let the pet leave the title bar for the composer, the sidebar and the side panel's tab bar while nothing needs you. Settings → Pet → Let it roam. |
| `ui.output_style` | string | unset | Name of an [output style](07-memory-and-context.md#output-styles). |
| `ui.task_report_view` | `"quiet"`, `"compact"`, `"detailed"` | `"quiet"` | How much the receipt under each turn shows: **Quiet** only a Verified, Unverified or Failed badge; **Compact** adds the changed files, duration and cost; **Detailed** adds tokens, the Not applicable badge and unfolded checks ([details](02-everyday-use.md#the-turn-receipt)). Settings → Appearance → Task report detail. |
| `compat.claude` | bool | `true` | Also read `.claude/` folders (agents, commands, skills) and `CLAUDE.md` files. `.claude/settings.json` is never read. |

```toml
[ui]
companion = "lively"

[ui.pet]
name = "Pip"
look = "lilac"
roam = false      # stay in the title bar (and on the project home)
```

The pet's level, streak and accessory are not settings: they are kept in
`pet.json` in the data folder ([Sessions and data](13-sessions-and-data.md#where-things-live)).

## Environment variables

| Variable | Effect |
|---|---|
| `ZENGINE_MODEL` | Overrides `model.main`. |
| `ZENGINE_BASE_URL` | Overrides `provider.base_url`. |
| `ZENGINE_PROVIDER` | Overrides `provider.kind` (`auto`, `openai_chat`, `anthropic`). |
| `ZENGINE_SHELL` | Overrides `shell.path`. |
| `ZENGINE_API_KEY` | API key for the configured provider (wins over all others). |
| `OPENROUTER_API_KEY`, `ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, `OPENCODE_API_KEY` | API key for that provider (wins over a stored key). |
| `BRAVE_API_KEY`, `TAVILY_API_KEY`, `EXA_API_KEY` | Web search keys. |
| `ZENGINE_CONFIG_DIR` | Use another config folder (settings, keys, trust, user extensions). |
| `ZENGINE_DATA_DIR` | Use another data folder (chats, checkpoints, cache, log). |
| `ZENGINE_GIT_BASH_PATH` | Windows: the Git Bash `bash.exe` to use. |
| `RUST_LOG` | Detail of the app log, for example `debug` (default `info`). |

Z Engine sets `ZENGINE=1` for commands it runs, and `ZENGINE_PROJECT_DIR`
and `CLAUDE_PROJECT_DIR` for hooks.

## Importing v1 settings

Z Engine 2.0 never changes v1 `config.toml` files, so a v1 installation
keeps working next to it.

- **User settings:** if `settings.toml` doesn't exist yet, the v1
  `~/.config/z-engine/config.toml` is converted once into a new
  `settings.toml` (with a comment saying so). Otherwise a commented default
  file is created.
- **Project settings:** a v1 `.z-engine/config.toml` is read and converted
  in memory while no `.z-engine/settings.toml` exists; the first change you
  make in Settings writes `settings.toml`.

The conversion moves `model`, `base_url`, `max_context_tokens`,
`max_output_tokens`, `compact_at_percent`, `max_task_continuations`,
`task_report_view` and `shell_path` to their v2 keys; turns v1 shell allow
entries such as `cargo test*` into `Bash(cargo test:*)`; maps the v1 hooks
`session_start` and `turn_completed` to `SessionStart` and `Stop`; and moves
`cost.overrides` to `[pricing]`. Anything it drops (such as `review`) is
reported. API keys in `auth.json` keep working; keys for custom endpoints
may need to be entered again because they are now stored per host.

See also: [Permissions and safety](03-permissions-and-safety.md) · [Sessions and data](13-sessions-and-data.md) · [Troubleshooting](14-troubleshooting.md)
