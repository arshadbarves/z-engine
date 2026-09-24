# Plan mode, questions and todos

Three features keep you in control of larger tasks: **plan mode** lets you
review a plan before anything changes, **structured questions** let the
agent ask you to choose between options, and the **todo list** shows its
progress step by step.

## Plan mode

In plan mode the agent researches and proposes; it changes nothing. It can
read and search files, run read-only commands such as `git log`, use the
language server, look things up on the web, and start `explore` or `plan`
subagents. Edits and commands that change anything are refused with "plan
mode is read-only".

### Enter plan mode

- Press **Shift+Tab** in the composer until the mode button reads **Plan**.
- Or choose **Plan** from the mode button under the composer.
- Or type `/mode plan`.
- To start every new chat in plan mode, set **Settings → Permissions →
  Permission mode** to **Plan** (`permissions.mode = "plan"`).

Then describe the task:

```text
Add rate limiting to the public API. Plan it first.
```

### The plan card

When the plan is ready, a **Plan ready for review** card appears with the
plan in markdown: the goal, the approach, the files to change, the ordered
steps, how it will be verified, and risks or open questions.

| Control | Effect |
|---|---|
| **Edit** / **Preview** | Switch to a text editor to change the plan yourself. An **edited** tag appears when your version differs. |
| **Approve & auto-accept edits** | Leave plan mode in **Auto-accept edits** mode and start implementing. |
| **Approve & ask before edits** | Leave plan mode in **Ask** mode and start implementing. |
| **Keep planning** | Opens a feedback box. Type what should change and click **Send feedback** (or press ⌘Enter / Ctrl+Enter). The agent stays in plan mode, revises, and proposes again. Esc closes the box. |

If you edited the plan, the agent is told to implement *your* version. While
the card waits, the status line says **Waiting for you**; you can still send
messages, which are queued.

> **Tip:** Plans are only proposed for work that changes files. If you ask
> a question in plan mode, the agent simply answers it.

Only the main agent proposes plans; the `plan` subagent returns its plan as
a report to the main agent instead.

## Questions from the agent

When a decision materially changes the result (two sound designs, a
destructive step, a product choice), the agent can ask you one to four
multiple-choice questions at once. A **Question** card appears:

- With several questions, tabs at the top switch between them; a check mark
  shows which are answered.
- For a single-choice question, clicking an option selects it and moves to
  the next question. For a multiple-choice question ("Pick any that
  apply"), click every option that applies.
- **Other…** lets you type your own answer. Press Enter in that box to
  submit once everything is answered.
- **Submit** sends your answers (it is enabled when every question has an
  answer).
- **Dismiss** declines to answer. The agent is told not to ask again and to
  continue on its best judgment or explain what is blocked.

Only the main agent can ask questions; subagents can't.

## Todos

For tasks with several steps, the agent keeps a todo list with the
`TodoWrite` tool. Each item is pending, in progress, or completed; the agent
keeps exactly one item in progress at a time.

- **The status line** at the top of the window shows the count (for
  example `2/5`) next to the current step, such as "Running the parser
  tests", and a ring around the companion fills as items are done.
- **Click the status line** to see the full checklist.
- **`/todos`** prints the list in the transcript as a checklist.
- A subagent's own todo list is shown in its transcript in the **Agents**
  panel.

The todo list survives [compaction](07-memory-and-context.md#automatic-compaction),
so it also serves as the agent's working memory in long sessions. If the
agent has worked for a while on a multi-step task without a list, it gets a
reminder to create one.

See also: [Permissions and safety](03-permissions-and-safety.md) · [Agents](04-agents.md) · [Everyday use](02-everyday-use.md)
