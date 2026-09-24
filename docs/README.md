# Z Engine documentation

Start with the part that matches what you want to do.

| I want to… | Read |
|---|---|
| use the app: set it up, run tasks, configure it | [User guide](user-guide/README.md) |
| understand how it works, with or without a programming background | [How Z Engine works](how-it-works/README.md) |
| look up a term | [Glossary](how-it-works/glossary.md) |
| change the code | [AGENTS.md](../AGENTS.md) (structure contract), [engine architecture](architecture/v2-engine.md), [style guide](engineering/style-guide.md), [GUI guide](design/gui-ui-guide.md) |
| keep the docs in sync with a change | [Documentation contract](AGENTS.md) |
| see what changed between versions | [Changelog](../CHANGELOG.md) |

## The user guide

Written for everyone who uses the app, including people who do not program.
It is organized by task, like Claude Code's documentation: getting started,
everyday use, permissions and safety, agents, plan mode, commands and
skills, memory and context, hooks, MCP and code intelligence, verification,
models and cost, a full settings reference, where your data lives, and
troubleshooting.

## How Z Engine works

Every feature and every crate is explained in three layers, so you can stop
reading when you have what you need:

1. **In plain words**: what it is, without jargon.
2. **How it works**: the mechanism, step by step.
3. **For developers**: the crates, files and types involved, and how to
   extend them.

## Other documents

- [status.md](status.md): how the v2 rewrite was delivered, and the release checks.
- Documents marked "Historical (v1)" describe the previous implementation,
  which has been removed. They are kept for reference only.
