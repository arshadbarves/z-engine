# Everyday use

This page covers the things you'll do most days: asking questions, fixing
bugs, adding features, reviewing and committing changes, attaching files,
steering the agent while it works, reading its output, and undoing work.

## Ask questions about code

Just ask. The agent searches and reads the project and answers with
references like `src/billing/invoice.rs:42`.

```text
Where do we validate email addresses, and which endpoints use it?
```

For broad questions the agent may start an `explore` subagent that searches
in parallel and reports back (see [Agents](04-agents.md)). Reading files in
the project never needs approval.

## Fix a bug

Describe the symptom and, if you have it, paste the error:

```text
`npm test` fails in auth.spec.ts with "token expired" even for fresh tokens.
Find the cause and fix it.
```

The agent typically reproduces the problem, edits the code, and runs the
tests again. The turn's badge tells you whether a passing test run is newer
than its last edit ([Verification](10-verification.md)).

## Add a feature

For anything larger than a few lines, consider plan mode first
(Shift+Tab until the mode reads **Plan**): the agent researches, proposes a
plan, and changes nothing until you approve it. See
[Plan mode, questions and todos](05-plan-mode-questions-and-todos.md).

## Review and commit

- `/review` reviews your uncommitted changes, or a target you name
  (`/review src/api`, `/review main...HEAD`). It uses the `review` agent and
  reports findings by severity; it does not change code.
- `/security-review` does the same with a security focus.
- `/commit` creates one commit in your repository's message style. It may
  run only `git status`, `git diff`, `git log`, `git add` and `git commit`
  for that turn, and it never pushes. Add a hint: `/commit only the parser
  changes`.

All built-in commands are listed in [Commands and skills](06-commands-and-skills.md).

## The composer

| You type | What happens |
|---|---|
| Text + **Enter** | Sends the message. **Shift+Enter** inserts a new line. |
| `@` | Lists matching project files and agents. |
| `/` | Lists slash commands, grouped as Prompts, Session and App. |
| `!command` | Runs a shell command yourself (Bash mode). |
| `# note` | Saves a note to an instruction file ([memory](07-memory-and-context.md#save-a-note-with-)). |
| **↑ / ↓** | Walk through your earlier messages (from the first or last line). |
| **Shift+Tab** | Cycle the permission mode: Ask → Auto-accept edits → Plan. |
| **Esc** | Cancel the running turn, or clear the draft when idle. |

In the pop-up lists, use ↑/↓ to move, Enter or Tab to pick, and Esc to
close the list.

### Attach files with @

Type `@` and part of a file name, then pick a file from the list. The file
is attached to your message as a chip above the input, and `@path` is
inserted into your text. When you send:

- text files are included (up to 256 KiB each) and count as already read,
  so the agent can edit them straight away;
- PDFs are sent as documents (up to 32 MiB);
- images are sent as images.

Remove an attachment with the × on its chip. Typing an `@path` by hand
without picking it from the list does not attach the file; the agent only
sees the text.

### Attach images

Paste an image, drop it onto the composer, or click the image button below
the input. Images are scaled down to at most 1568 pixels on the long side
before sending. You can attach up to six images per message.

> **Note:** Attachments are only sent with a new prompt. If you send while
> the agent is working, the attachments stay in the composer for your next
> prompt.

### Run a shell command yourself

Start a message with `!` to run a command in the project without involving
the model, for example `!git status`. The output appears in the
**Terminal** drawer above the composer (Esc hides it; a terminal button
brings it back). These commands run directly on your machine, without
approval prompts and outside the sandbox, with a 10-minute limit. A command
that isn't read-only counts as a change for the next turn's badge.

## Steer, interrupt, cancel

You don't have to wait for the agent to finish.

- **Steer:** type and press **Enter** while it works. The message is
  queued (shown as pills labelled *queued* above the input) and delivered
  at the next step, between tool calls. Click a pill to edit it, or × to
  remove it. In the transcript it appears as a **Steered** note.
- **Interrupt:** press **⌘Enter** (macOS) or **Ctrl+Enter** (Windows,
  Linux), or click **Interrupt**. The current step stops and your message
  is sent right away as a new turn. The agent is told it was interrupted.
- **Cancel:** press **Esc** or the stop button. The turn ends; unfinished
  tool calls are cancelled. Background jobs keep running. Messages still
  queued are kept and sent with your next prompt.

Slash commands that start a turn (like `/review`) can't be queued: wait
until the agent is idle.

## The todo strip

For multi-step work the agent keeps a todo list. A strip above the
composer shows progress (for example `3/7`), a bar, and the current item.
Click it to expand the full checklist. See
[Plan mode, questions and todos](05-plan-mode-questions-and-todos.md#todos).

## Reading tool cards

Every tool call is a one-line card: an icon, the tool name, its subject (a
file path, command or URL), and the time it took. The dot and tag show the
status: running, done, failed, **denied** (by you, a rule or a hook) or
**cancelled**. Click a card to expand it:

- **Edit/Write** cards show the diff with added and removed line counts.
- **Bash** cards show the command, its live output, and the exit code.
- **Agent** cards show the subagent's type, model, tokens, cost and a
  **Open transcript** link.
- Other cards show the tool's output.

Under each turn, the footer shows the verification badge, a note if the
turn was cancelled, failed or stopped by a budget, and the duration, tokens
(`in · out`) and cost.

## The diff panel

Click **Review changes** in the top bar (⌘D / Ctrl+D) to open the review
panel. It has two scopes:

- **Chat** – every file that changed in the project since this chat's first
  code checkpoint.
- **Git** – all uncommitted changes compared with your last commit.

Pick a file in the list (you can filter it) to see its diff. With several
files, the arrow buttons step through them. Drag the panel's left edge to
resize it.

## Rewind

Hover over one of your messages and click **Rewind**. Choose:

| Option | Effect |
|---|---|
| **Code and conversation** | Restore the files to how they were before this message, and drop this message and everything after it. |
| **Conversation only** | Drop this message and everything after; keep the files as they are now. |
| **Code only** | Restore the files; keep the conversation. |

When you rewind the conversation, your message goes back into the composer
so you can edit and resend it. A running turn is cancelled first. A
notification lists which files were restored, deleted, or left alone.

> **Warning:** Rewind restores files, not the outside world. Installed
> packages, database changes, pushes and other side effects of commands are
> not undone. Files ignored by git and files larger than 8 MiB are not part
> of checkpoints. Details: [Sessions and data](13-sessions-and-data.md#checkpoints-and-rewind).

## Resume and manage chats

- Every chat is saved automatically. Click it in the sidebar to reopen it,
  also after restarting the app.
- ⌘K / Ctrl+K opens the command palette, which lists recent chats, your
  workspaces and common actions. `/resume` opens it with chats only.
- The sidebar shows a spinner for chats that are working and a shield for
  chats waiting for your approval. If a chat in the background needs you,
  a pop-up offers **Open**.
- `/clear` or **New chat** starts a fresh chat; the old one stays.

> **Warning:** The trash icon next to a chat deletes it immediately,
> without asking. Removing a workspace from the sidebar also deletes all of
> its chats. Your project files are not touched.

## Export a transcript

`/export` copies the chat to your clipboard as Markdown (tool calls
summarized, internal reminders hidden). `/export json` copies every
recorded event as JSON. Both are also in the command palette.

See also: [Permissions and safety](03-permissions-and-safety.md) · [Commands and skills](06-commands-and-skills.md) · [Sessions and data](13-sessions-and-data.md)
