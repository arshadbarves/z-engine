# Everyday use

This page covers the things you'll do most days: starting from the project
home, asking questions, fixing bugs, adding features, reviewing and
committing changes, attaching files, steering the agent while it works,
following its progress, and undoing work.

## The project home

With no chat open, the main area shows the active project's home: the
project name and git branch, **What should we work on?**, and the composer
in the middle. Below the composer:

- **Starter chips** fill the composer with a first prompt that fits the
  project, such as **Review my 3 uncommitted changes**, **Write an AGENTS.md
  for this project** or **Explain how this project works**. **More ideas**
  shows the rest.
- **Continue** lists your three latest chats in the project with where each
  stands, its age and its cost.
- **Uncommitted changes** lists the first changed files; click one, or
  **Review all N**, to open them in the [Changes panel](#review-changes-in-the-changes-panel).
- **Set up this project** lists what is still missing (**Connect a model**,
  **Trust this project**, **Add an AGENTS.md**), each with a button, and
  disappears once everything is done.

When you send the first message, the composer glides to the bottom and the
chat begins. **Home** in the sidebar and **New chat** (⌘N / Ctrl+N) bring
you back here. Without any project, the home offers **Add a project**.

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
(Shift+Tab until the mode chip reads **Plan**): the agent researches,
proposes a plan, and changes nothing until you approve it. See
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

To read the changes yourself, open the
[Changes panel](#review-changes-in-the-changes-panel). All built-in commands
are listed in [Commands and skills](06-commands-and-skills.md).

## The composer

The composer's bar holds the **+** menu (**Attach images**, **Mention a
file or agent**, **Run a command**, **Save a note to memory**, **Run a shell
command yourself**, and **Show the terminal** when shell output is hidden;
each item types its character for you), the mode chip
([permission modes](03-permissions-and-safety.md#permission-modes)), the
model chip with the reasoning effort
([choosing a model](11-models-providers-and-cost.md#choosing-a-model-for-a-chat)),
and Send. While the agent works, a square **Stop** button appears.

| You type | What happens |
|---|---|
| Text + **Enter** | Sends the message; while the agent works, queues it ([below](#steer-interrupt-cancel)). **Shift+Enter** inserts a new line. |
| `@` | Lists matching project files and agents. |
| `/` | Lists slash commands, grouped as Prompts, Session and App. |
| `!command` | Runs a shell command yourself (shell mode). |
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

Paste an image, drag it onto the composer (it shows **Drop images to attach
them**), or choose **Attach images** in the **+** menu. Images are scaled
down to at most 1568 pixels on the long side before sending. You can attach
up to six images per message.

> **Note:** Attachments are only sent with a new prompt. If you send while
> the agent is working, the attachments stay in the composer for your next
> prompt.

### Run a shell command yourself

Start a message with `!` to run a command in the project without involving
the model, for example `!git status`; the composer shows a **Shell** tag
and a **Run** button. The output appears in the **Shell** drawer that rises
from the top of the composer, with buttons to copy it, make the drawer
taller, clear it and hide it (Esc). **Show the terminal** in the **+** menu
brings it back.

These commands run directly on your machine, without approval prompts and
outside the sandbox, with a 10-minute limit. A command that isn't read-only
counts as a change for the next turn's badge.

## Steer, interrupt, cancel

You don't have to wait for the agent to finish. While it works, the
placeholder reads "Add to what the agent is doing…", and once you type, the
composer shows **Enter queues · ⌘Enter interrupts** (Ctrl+Enter on Windows
and Linux).

- **Steer:** type and press **Enter**. The message is queued (shown as
  pills labelled *queued* above the input) and delivered at the next step,
  between tool calls. Click a pill to edit it, or × to remove it. In the
  transcript it appears as a **Steered** note.
- **Interrupt:** press **⌘Enter** (macOS) or **Ctrl+Enter** (Windows,
  Linux), or click that part of the hint. The current step stops and your
  message is sent right away as a new turn. The agent is told it was
  interrupted.
- **Cancel:** press **Esc** or the square **Stop** button. The turn ends;
  unfinished tool calls are cancelled. Background jobs keep running.
  Messages still queued are kept and sent with your next prompt.

Slash commands that start a turn (like `/review`) can't be queued: wait
until the agent is idle.

## The title bar

The middle of the title bar is the **island**: the companion (a small glass
orb) and one line with at most one number. While the agent works, the line
says what it is doing right now ("Reading auth.rs", "Running the parser
tests"), the number is the elapsed time, and a ring around the orb fills as
the todo list gets done. When a turn ends, the line briefly shows the
result, such as **Verified** or **Checks failed**, and the turn's duration.
It says **Provider busy** with a countdown while a request is retried,
turns amber when this chat needs you (an approval, a question or a plan;
clicking the island then scrolls to the card), and shows the chat's title
when idle.

Click the island to open it: the whole message, the latest steps (**Now**),
the plan with how many items are done, running helpers (**Open** shows the
agents panel), this turn's and the chat's cost, and recent warnings (a small
dot on the island means some are new). Long
notices, errors and notices with buttons open it by themselves; it closes
when the notice ends, unless the pointer is over it. **Open in Inbox** shows
a notice in the [Inbox](#the-inbox).

Left of the island, an amber number counts other chats that need you:
click it to open that chat, or the Inbox when there are several. Right of
it, a ring shows how full the context is, with the percentage from 65%
(amber, red from 85%); click it for the
[context card](07-memory-and-context.md#the-context-window). At the right
of the title bar, the changes button (**Review changes**, ⌘D / Ctrl+D)
shows how many files this chat has changed and opens the
[Changes panel](#review-changes-in-the-changes-panel). While the sidebar is
hidden (⌘B / Ctrl+B), **New chat**, search and **Settings** move to the
title bar.

## The Inbox

**Inbox** in the sidebar collects what is going on across all your chats:

- **Needs you**: approvals, questions, plans and trust requests. Approvals
  can be answered right here with **Allow once** or **Deny**; **Open chat**,
  **Answer**, **Review plan** and **Decide** open the chat.
- **Finished while you were away**: chats whose turn ended while you were
  elsewhere, with how each ended; **Open** opens one.
- **Notices**: every notice in full, including each chat's warnings, errors
  and blocked hooks, newest first. Long ones fold (**Show all**), each has a
  copy button, and **All** / **Problems** filters them.

The Inbox badge counts what needs you, finished chats you haven't opened,
and problems since you last looked; opening the Inbox marks them as read.
With nothing waiting it says **You're all caught up**. The Inbox is kept
only while the app runs ([Sessions and data](13-sessions-and-data.md#where-things-live)).

## Reading tool runs and cards

Two or more tool calls in a row fold into one line that says what they did,
for example "Read 2 files · searched 1× · edited 1 file · ran 1 command",
with the step in progress while it runs, flags such as **1 failed** or **1
denied**, and the number of steps. Under the line, a running command shows
its last three lines, and a failure the last three lines of its output.
Click the line to see each call's own card; agent, question and plan calls
always keep their own.

Every call is a one-line card: an icon, the tool name, its subject (a file
path, command or URL) and the time it took. The dot and tag show the
status: running, done, failed, **denied** (by you, a rule or a hook) or
**cancelled**. Click a card to expand it:

- **Edit/Write** cards show the diff with added and removed line counts;
  they open by themselves only when the edit failed.
- **Bash** cards show the command, its live output and the exit code; they
  open by themselves only on an error. While a command runs, its last three
  lines show under the card.
- **Agent** cards show the subagent's type, task, status and time, with
  **Open** for its transcript. Click the line for its model, tokens, cost,
  tool calls, worktree state and result.
- Other cards show the tool's output.

### The turn receipt

Under each turn, a one-line *receipt* sums up how it ended. How much it
shows is set in **Settings → Appearance → Task report detail**
(`ui.task_report_view`):

| Detail | The receipt shows |
|---|---|
| **Quiet** (default) | The verification badge, only when it is **Verified**, **Unverified** or **Failed** ([Verification](10-verification.md)). |
| **Compact** | Also the files the turn changed (four chips, then **+N**; click one to see it in the Changes panel), the duration and the cost. |
| **Detailed** | Also the tokens, the **Not applicable** badge of turns that changed nothing, and the checks unfolded. |

Notes such as **Cancelled**, **Failed**, **Stopped** or **Interrupted**
always show. Hover a receipt for the turn's time, tokens and cost.

## Review changes in the Changes panel

Click the changes button in the title bar (⌘D / Ctrl+D) to open the
**Changes** panel beside the chat; its header counts the files and the
lines added and removed. Choose **This chat** (every file that changed in
the project since this chat's first code checkpoint) or **Uncommitted**
(all uncommitted changes compared with your last commit), and **Unified**
or **Split** (one column, or old and new side by side; remembered on this
computer). **Use the whole window** and **Dock beside the chat** switch its
size; drag its left edge to resize it.

With several files, a list on the left groups them by folder, marks each
**A**, **M**, **D** or **R** (added, modified, deleted, renamed) with its
line counts, and offers a filter when there are more than six. Each file's
bar has **Open** (in its default app), a button that copies the diff, and a
**…** menu: **Reveal in Finder** (**Show in Explorer** on Windows, **Show
in file manager** on Linux), **Copy path**, **Copy relative path** and
**Copy diff**. Long unchanged stretches fold into **Show N unchanged
lines**; very long diffs stop after 400 lines with **Show the remaining N
lines**.

| Key (while you're not typing) | Effect |
|---|---|
| `]` or `j` / `[` or `k` | Next file / previous file |
| **Esc** | Leave the whole-window view, then close the panel |

A finished turn refreshes the panel, and a file chip in a receipt or on the
project home opens it at that file.

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
- The sidebar lists chats under their project, with their age (`5m`, `3h`,
  `2d`). A dot before a chat means it is working, needs you (amber),
  finished while you were away, or its last response didn't complete; hover
  the chat for the exact state. The open chat shows no dot, because the title bar
  reports it. A project lists eight chats, then **Show N more**; chats whose
  folder isn't a project are under **Other chats**.
- Click a project to fold or unfold it. Hover it for **+** (a new chat
  there), or right-click it for **New chat here**, **New chat in a
  worktree…**, **Reveal in Finder**, **Copy path** and **Remove project…**.
- ⌘K / Ctrl+K, or **Search** in the sidebar, opens the
  [command palette](06-commands-and-skills.md#the-command-palette): actions,
  recent chats, projects and settings in one search. `/resume` opens it with
  chats only.
- `/clear` or **New chat** starts a fresh chat; the old one stays.

> **Warning:** Deleting a chat (the trash icon next to it) asks you to
> confirm, then removes it for good. Removing a project from the sidebar
> also deletes all of its chats once you confirm. Your project files are
> not touched.

## Export a transcript

`/export` copies the chat to your clipboard as Markdown (tool calls
summarized, internal reminders hidden). `/export json` copies every
recorded event as JSON. Both are also in the command palette (**Copy the
chat as Markdown**, **Copy the chat as JSON**).

See also: [Permissions and safety](03-permissions-and-safety.md) · [Commands and skills](06-commands-and-skills.md) · [Sessions and data](13-sessions-and-data.md)
