# Getting started

This page takes you from download to your first finished task: installing
Z Engine, connecting it to an AI model provider, opening a project folder,
and approving the agent's first actions.

## Install Z Engine

Download the installer for your system from
[GitHub Releases](https://github.com/arshadbarves/z-engine/releases).

- **macOS:** choose the Apple Silicon or Intel `.dmg`, open it, and drag
  **Z Engine** to your Applications folder.
- **Windows:** run the `.exe` or `.msi` installer.
- **Linux:** install the `.deb` or `.rpm` package, or download the
  `.AppImage`, make it executable (`chmod +x`), and run it.

> **Tip:** If macOS refuses to open the app the first time, Control-click
> it in Applications and choose **Open**.

Two free tools make Z Engine better if they are installed:

- **git** – needed for code checkpoints (undo), the diff panel and agent
  worktrees. Without git, those features are off.
- **ripgrep** (`rg`) – faster code search. Without it, search uses a
  built-in engine.

Z Engine updates itself: when a new version is available, a version chip
appears in the top bar. Click it (or **Settings → About & Updates →
Update & Restart**) to install. The app checks for updates when it starts.

## Connect a model provider

A *model provider* is the service that runs the AI model. Z Engine needs one
before it can do anything.

1. Open **Settings** (⌘, on macOS, Ctrl+, on Windows and Linux) and choose
   **Providers**.
2. Click **Connect** next to a provider.
3. Paste an API key if the provider needs one (see below). An *API key* is a
   secret string the provider gives you so it can bill your account.
4. Check the **Main model** field; it is filled with a sensible default.
5. Click **Connect & Set Active**.

The provider becomes **Active**. Keys are stored in a separate file,
`auth.json`, never in settings files. Each key belongs to one API host and
is only sent to that host.

| Provider | Key | Notes |
|---|---|---|
| OpenRouter | Required | The default. One key gives access to hundreds of models. |
| Anthropic | Required | Claude models through Anthropic's own API, with prompt caching and extended thinking. |
| OpenAI | Required | GPT and o-series models. |
| OpenCode Zen | Optional | Free chat models work **without a key**. A key is only needed for paid models. |
| Google AI (Gemini), DeepSeek, Groq, Mistral AI | Required | OpenAI-compatible endpoints. |
| Ollama (Local) | None | A model running on your computer at `http://localhost:11434/v1`. |
| LM Studio (Local) | None | LM Studio's local server at `http://localhost:1234/v1`. |
| Custom endpoint | Usually | Any OpenAI-compatible or Anthropic-compatible server or proxy. |

Each dialog has a **Get API Key** button that opens the provider's key page.
Under **Endpoint** you can change the API base URL, the wire format (Auto,
OpenAI-compatible, Anthropic), extra HTTP headers, and prompt caching.

### Try it without an account: OpenCode Zen

1. In **Settings → Providers**, click **Connect** next to **OpenCode Zen**.
2. Leave the key empty and click **Connect & Set Active**.

You can now chat with Zen's free models.

### Use a model on your own computer

1. Start Ollama or LM Studio's local server and load a model.
2. Click **Connect** next to **Ollama (Local)** or **LM Studio (Local)**.
3. In **Main model**, type the model name your server serves (for example
   `llama3.3` for Ollama). LM Studio has no default, so the field starts
   empty.
4. Click **Connect & Set Active**.

> **Note:** Local models vary a lot in how well they use tools. If the agent
> gets confused, try a larger model.

More about keys, environment variables and model choice:
[Models, providers and cost](11-models-providers-and-cost.md).

## Open a project

Z Engine works inside a *workspace*: a project folder on your disk.

1. In the sidebar, click **+** next to **Workspaces** (or **Add folder**).
2. Choose your project's folder.

The folder appears in the sidebar and becomes the active workspace. Click
**New chat** (⌘N / Ctrl+N) to start a conversation there. If you have no
workspace yet, **New chat** asks you for a folder first.

> **Tip:** Z Engine works best in a git repository. Run `git init` in a new
> project so checkpoints and the diff panel work.

If the project's own settings would run programs or loosen yours (hooks,
MCP servers, checks, permission rules and a few more; see
[workspace trust](03-permissions-and-safety.md#workspace-trust)), a banner
asks whether you trust the workspace. Choose **Not now** if you are unsure;
everything else still works.

## Your first task

1. Type a request in the composer, for example:

   ```text
   Explain what this project does and where the entry point is.
   ```

2. Press **Enter** to send. (Shift+Enter adds a new line.)
3. Watch the transcript. Each tool the agent uses (Read, Grep, Bash, ...)
   appears as a card with a status and duration. Click a card to expand its
   details.
4. When the agent finishes, a footer under the turn shows how long it took,
   the tokens used and the cost.

Reading files inside the project and running read-only commands such as
`ls` or `git status` do not need approval, so a question like this usually
runs without interruption.

Now ask for a change:

```text
Add a --version flag to the command-line parser and a test for it.
```

This time the agent needs to edit files and run commands, so it asks first.

## The approval card

An *approval card* appears in the transcript whenever the agent wants to do
something your permission settings don't already allow. It shows:

- the tool (for example **Edit** or **Bash**) and a one-line title such as
  `Edit src/cli.rs`;
- why approval is needed (for example "edits src/cli.rs" or "the command
  needs approval");
- a preview: the diff of an edit, or the exact command;
- the rule that "always allow" would add, for example `Bash(cargo test:*)`.

Answer with a button or a key (the card takes keyboard focus when the
composer is empty):

| Button | Key | Effect |
|---|---|---|
| **Allow once** | `y` | Run this one action. |
| **Allow for session** | `s` | Run it and allow matching actions for the rest of this chat. |
| **Always for project** | `p` | Also save the rule to your personal project settings. |
| **Deny…** | `n` | Refuse. You can type what the agent should do instead; press Enter to send. |

While a card waits, the agent is paused and the status line says **Waiting
for you**. You can still type messages; they are queued until the agent
continues. Details and rule syntax: [Permissions and safety](03-permissions-and-safety.md).

> **Tip:** If you trust the agent with edits in this project, switch the
> permission mode to **Auto-accept edits** with Shift+Tab. Commands still
> ask.

## Where to go next

- Learn the daily workflow in [Everyday use](02-everyday-use.md).
- Stop repeated prompts with rules in
  [Permissions and safety](03-permissions-and-safety.md).
- Tell the agent about your project in [Memory and context](07-memory-and-context.md)
  (try `/init`).

See also: [Everyday use](02-everyday-use.md) · [Models, providers and cost](11-models-providers-and-cost.md) · [Troubleshooting](14-troubleshooting.md)
