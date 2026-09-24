# Troubleshooting

This page helps you find out what's wrong and fix the most common problems:
start with `/doctor` and the log file, then look up your symptom below.

## Start with /doctor

Type `/doctor` in a chat. It checks, without using the model:

- the provider, its address, and whether an API key is present;
- the model and its context window;
- whether the model catalog is loaded;
- whether git and ripgrep are installed;
- language servers (configured, starting, running or failed);
- MCP servers and their state;
- how many hooks are configured;
- whether the workspace is trusted, and what stays off if not;
- the verification mode and the checks found.

`/status`, `/mcp` and `/context` give more detail on the session, MCP
servers and context use.

## The app log

The log is `z-engine-gui.log` in the data folder:

- macOS: `~/Library/Application Support/z-engine/z-engine-gui.log`
- Linux: `~/.local/share/z-engine/z-engine-gui.log`
- Windows: `%APPDATA%\z-engine\z-engine-gui.log`

For more detail, start Z Engine with the environment variable
`RUST_LOG=debug`. Notifications shown in the app (pop-ups) are usually
the quickest clue.

## Common problems

### No models are listed in the model picker

- The picker shows only the **active provider's** models. If that provider
  needs a key and has none, the list is empty: add the key in **Settings →
  Providers**.
- Local servers (Ollama, LM Studio) and some custom endpoints aren't in the
  catalog. Type the model name in **Custom model ID** at the bottom of the
  picker.
- The catalog is downloaded from models.dev the first time you open the
  picker. If you were offline, it retries the next time you open it.
- The catalog is cached and not refreshed automatically. To get newly
  released models, quit Z Engine, delete `cache/models-dev.json` in the data
  folder, and start again. You can always use a model by typing its id.

### "provider returned HTTP 401" or other key errors

- Check **Settings → Providers**: the active provider should say
  **Active**, not **Active · needs a key**.
- Keys are stored per API host. A custom endpoint or a changed base URL
  needs its own key.
- An environment variable wins over a stored key: `ZENGINE_API_KEY` first,
  then `OPENROUTER_API_KEY`, `ANTHROPIC_API_KEY`, `OPENAI_API_KEY` or
  `OPENCODE_API_KEY`. A stale variable can hide the key you stored.
- "stored API keys are unreadable" means `auth.json` is damaged; Z Engine
  won't overwrite it. Fix or remove the file and enter the key again.
- If the model id is wrong for the provider, the provider reports an
  error; check the id format (for example `anthropic/claude-sonnet-4.5` on
  OpenRouter, `claude-sonnet-4-5` on Anthropic).

### Rate limits, "Provider busy", retrying

Z Engine retries temporary failures (HTTP 408, 429, 500, 502, 503, 504,
529, timeouts, lost connections) up to 5 attempts, showing **Provider busy ·
retry N in Xs**. If it still fails, the turn ends with the provider's error.

- Wait and send again, or choose another model.
- Configure `model.fallbacks` so another model takes over automatically
  ([Models, providers and cost](11-models-providers-and-cost.md#fallback-models-and-retries)).
- Many parallel subagents use the provider heavily; lower
  `agents.max_concurrent`.

### A tool was denied

Expand the tool card; it says why:

- "denied by rule ..." – a deny rule in one of your settings files.
- "plan mode is read-only" – switch modes (Shift+Tab) or approve a plan.
- "A PreToolUse hook blocked this call" – a hook refused it; `/hooks`
  shows recent hook runs.
- "The user denied this action" – someone answered **Deny**.
- "this agent works in the isolated worktree ..." – a worktree agent tried
  to change your main project.

**Settings → Permissions** shows all rules from all files. Rules that don't
parse are skipped with a "ignored permission rule" notice.

### An MCP server fails

- Run `/mcp`: it shows the error and the last lines of the server's error
  output.
- Use **Test** in **Settings → MCP**.
- "not found on PATH": use the full path of the program in `command`.
- The server doesn't see your shell's variables; put tokens in `env`
  ([details](09-mcp-and-code-intelligence.md#in-a-settings-file)).
- A server defined in the project's settings doesn't start until you
  [trust the workspace](03-permissions-and-safety.md#workspace-trust).
  `/mcp` says so.
- Slow servers: raise `timeout_secs`.

### A language server doesn't start

- Install the server program and make sure it's on the `PATH` Z Engine
  sees (for example `rust-analyzer`, `typescript-language-server`,
  `pyright`, `gopls`, `clangd`).
- Servers start on the first file the agent asks about; `/doctor` lists
  "none started yet" until then.
- Check `lsp.enabled` and that the server isn't disabled in
  `[lsp.servers]`.
- For rust-analyzer and gopls, the project needs its root file
  (`Cargo.toml`, `go.mod`).

### "The command sandbox is enabled but unavailable"

- Linux: install bubblewrap (`bwrap`) from your package manager.
- macOS: the built-in `sandbox-exec` can't start inside another sandbox
  (for example when Z Engine itself runs sandboxed).
- Windows: there is no sandbox yet.

Until it's fixed, commands run unconfined and ask for approval.

### Commands fail with "command not found"

Commands only get a few environment variables, and apps started from the
Dock, Start menu or a launcher often have a shorter `PATH` than your
terminal. Set the path explicitly:

```toml
[shell.env]
PATH = "/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin"
```

Add other variables your tools need to `shell.env_passthrough`, for
example `["SSH_AUTH_SOCK", "JAVA_HOME"]`. To use another shell, set
`shell.path`.

### The first request is slow

On the first message of a chat, Z Engine takes the first code checkpoint
(hashing every project file; the agent's tools wait for it), builds the
repository map, and starts MCP servers. In large projects this can take a
while; later messages are faster, and prompt caching makes repeated
context cheaper and quicker. Projects with more than 50,000 files skip
checkpoints.

### I edited a file by hand and nothing changed

Settings changed in the Settings screens apply to open chats at once.
Files you edit by hand (settings, agents, commands, skills, rules,
instructions) are read when a chat opens; start a new chat or restart
Z Engine. A settings file with an error is skipped: the Settings screen
shows "This file is not applied until it is fixed".

### The agent stopped by itself

The turn footer or a notification explains why:

- "Stopped · reached the limit of 200 model turns" – raise
  `agents.max_turns`.
- "Stopped · reached the session cost cap" – raise or clear
  `agents.session_cost_cap_usd`.
- "Verification ended after N automatic continuation(s)" – auto/strict mode
  used its budget.
- "Interrupted · the app closed mid-turn" – send your request again.

### Rewind is greyed out

"No code checkpoint for this prompt" means checkpoints were off: git isn't
installed, the project has more than 50,000 files, or the snapshot failed
(a notice said why). **Conversation only** still works.

### Keys don't answer the approval card

The `y`/`s`/`p`/`n` keys work when the card has focus. Click the card
(not a button), then press the key.

## FAQ

**Can I use Z Engine offline?** With a local model server (Ollama, LM
Studio), yes. Web tools, catalog download and update checks need a network.

**Does it use my Claude Code setup?** It reads `.claude/agents`,
`.claude/commands`, `.claude/skills` and `CLAUDE.md` files. It does **not**
read `.claude/settings.json`; put permissions and hooks in
`.z-engine/settings.toml`.

**Does Z Engine 2.0 change my v1 settings or chats?** No. v1 files are only
read. See [Settings reference](12-settings-reference.md#importing-v1-settings).

**Can the agent push to my remote?** Only if you allow the command. The
agent is instructed never to push unless you ask; add
`ask = ["Bash(git push:*)"]` to be sure.

**How do I stop everything?** Esc cancels the turn and its foreground
subagents. Background jobs and agents are stopped from the **Jobs** tab.
Quitting the app stops all of them.

**Is there a command-line version?** No. Z Engine 2.0 is a desktop app.

See also: [Getting started](01-getting-started.md) · [Settings reference](12-settings-reference.md) · [Sessions and data](13-sessions-and-data.md)
