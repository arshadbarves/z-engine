# docs/AGENTS.md — Documentation contract

Read this before changing any feature, and whenever you edit a file under
`docs/`. It applies to every agent and human. The root [AGENTS.md](../AGENTS.md)
requires it: **a change that adds, changes or removes anything a user or a
contributor can observe updates the documentation in the same change.**
The `docs-maintainer` subagent ([.claude/agents/docs-maintainer.md](../.claude/agents/docs-maintainer.md))
follows this file and can be asked to do the update.

## 1. When the docs must change

Update the docs when a change touches any of these:

- something the user sees or does in the app (a screen, card, button,
  shortcut, message, flow);
- a slash command, tool, built-in agent, hook event or hook field;
- a settings key, its default, its allowed values, or a settings file or
  environment variable;
- permission behavior (modes, rules, what always asks, trust, sandbox);
- where data is stored, what is sent to a provider, or cost behavior;
- a protocol event or command, a crate, or which crate owns what.

Internal refactors with no observable effect need no user-guide change, but
update [how-it-works/crates.md](how-it-works/crates.md) when a crate's main
modules or responsibilities move. Never document planned or unshipped
behavior as if it works; remove documentation of removed features.

## 2. Where things are documented

| Document | Audience | Covers |
|---|---|---|
| [user-guide/](user-guide/README.md) | people using the app, including non-programmers | how to do things, every command, setting and file the user touches |
| [how-it-works/](how-it-works/README.md) | everyone: plain words first, then mechanism, then developer detail | how each feature works and what each crate does |
| [architecture/v2-engine.md](architecture/v2-engine.md) | engine contributors | the runtime contract (round loop, gate, stop boundary, persistence, hooks) |
| [../AGENTS.md](../AGENTS.md) | contributors and coding agents | crate layout, dependency rules, file budget, how to add things |
| [engineering/style-guide.md](engineering/style-guide.md), [design/gui-ui-guide.md](design/gui-ui-guide.md) | contributors | code and UI conventions |
| [../README.md](../README.md) | first-time visitors | highlights, install, quick start, pointers |
| [../CHANGELOG.md](../CHANGELOG.md) | everyone | every user-visible change, under `[Unreleased]` until a release |

## 3. What to update for each kind of change

| When you change… | Update |
|---|---|
| a tool (new, renamed, new input, new limit) | its description `crates/z-engine-prompts/prompts/tools/<snake_name>.md`; the user-guide page that explains the workflow; [how-it-works/features-core.md](how-it-works/features-core.md) (tools); the README tools list |
| a settings key or default | [user-guide/12-settings-reference.md](user-guide/12-settings-reference.md) and the feature's user-guide page; the commented example in `crates/z-engine-config/src/default_config.toml`; the how-it-works page when behavior changes |
| a slash command | [user-guide/06-commands-and-skills.md](user-guide/06-commands-and-skills.md); [how-it-works/features-interaction.md](how-it-works/features-interaction.md); the README commands paragraph |
| a built-in or custom-agent feature | [user-guide/04-agents.md](user-guide/04-agents.md); [how-it-works/features-agents-and-context.md](how-it-works/features-agents-and-context.md) |
| hooks (event, input field, output field) | [user-guide/08-hooks.md](user-guide/08-hooks.md); [architecture/v2-engine.md](architecture/v2-engine.md#hooks); [how-it-works/features-interaction.md](how-it-works/features-interaction.md) (hooks) |
| permissions, trust or the sandbox | [user-guide/03-permissions-and-safety.md](user-guide/03-permissions-and-safety.md); [how-it-works/features-core.md](how-it-works/features-core.md) and [features-integrations-and-safety.md](how-it-works/features-integrations-and-safety.md) |
| plan mode, questions, todos, steering | [user-guide/05-plan-mode-questions-and-todos.md](user-guide/05-plan-mode-questions-and-todos.md) or [02-everyday-use.md](user-guide/02-everyday-use.md); [how-it-works/features-interaction.md](how-it-works/features-interaction.md) |
| memory, rules, context, compaction, caching | [user-guide/07-memory-and-context.md](user-guide/07-memory-and-context.md); [how-it-works/features-agents-and-context.md](how-it-works/features-agents-and-context.md) |
| MCP or language servers | [user-guide/09-mcp-and-code-intelligence.md](user-guide/09-mcp-and-code-intelligence.md); how-it-works integrations page |
| verification | [user-guide/10-verification.md](user-guide/10-verification.md); how-it-works integrations page |
| providers, models, cost | [user-guide/11-models-providers-and-cost.md](user-guide/11-models-providers-and-cost.md); how-it-works integrations page |
| sessions, data locations, checkpoints, rewind | [user-guide/13-sessions-and-data.md](user-guide/13-sessions-and-data.md); how-it-works integrations page |
| a UI surface or shortcut | the user-guide page for that flow; [how-it-works/features-desktop-screens.md](how-it-works/features-desktop-screens.md) (the window and engine link: [features-desktop-app.md](how-it-works/features-desktop-app.md)) |
| a protocol event/command, a crate, crate ownership | [how-it-works/crates.md](how-it-works/crates.md) (and its diagram); [../AGENTS.md](../AGENTS.md); [architecture/v2-engine.md](architecture/v2-engine.md) |
| an error message, a new failure mode, a diagnostic | [user-guide/14-troubleshooting.md](user-guide/14-troubleshooting.md) |
| anything user-visible | an entry under `## [Unreleased]` in [../CHANGELOG.md](../CHANGELOG.md) (Added / Changed / Fixed / Removed) |

A new page goes into its folder's `README.md` table of contents and the
website sidebar (section 6), and gets a "See also" link from related pages.
A new term goes into [how-it-works/glossary.md](how-it-works/glossary.md).

## 4. Facts come from the code

Before writing a fact, confirm it where it is defined:

- settings keys, defaults and clamps: `crates/z-engine-config/src/settings/*.rs`,
  `settings/normalize.rs`, `default_config.toml`; file locations and
  environment variables: `crates/z-engine-config/src/paths.rs`, `loader.rs`,
  `credentials.rs`;
- tools: `crates/z-engine-tools/src/builtin/*.rs` and `crates/z-engine-prompts/prompts/tools/*.md`;
- commands: `crates/z-engine-engine/src/commands/`, `crates/z-engine-prompts/prompts/commands/*.md`,
  UI commands in `crates/z-engine-gui/ui/src/lib/stores/uiCommands.ts`;
- agents: `crates/z-engine-prompts/prompts/agents/*.md`, `crates/z-engine-config/src/extensions/agent.rs`,
  `crates/z-engine-engine/src/orchestration/`;
- permissions: `crates/z-engine-policy/src/rules/` and `src/decide/`;
- hooks: `crates/z-engine-engine/src/hooks/`, `crates/z-engine-config/src/settings/hooks.rs`;
- events and commands between app and engine: `crates/z-engine-protocol/src/`;
- UI surfaces and shortcuts: `crates/z-engine-gui/ui/src/components/`, `ui/src/lib/domain/composerKeys.ts`.

If the code and a document disagree, the code wins: fix the document, or
report the disagreement if the code looks wrong.

## 5. How to write

**User guide** (Claude Code docs style): second person; lead with the task;
numbered steps; a concrete example (TOML, markdown, script) for anything
configurable; tables for reference lists; callouts as `> **Note:**`,
`> **Tip:**`, `> **Warning:**`; explain a technical term the first time it
appears on a page; no crate or type names unless the user types them;
end with `See also:` links.

**How it works** (mixed level): every section has three parts, in order —
**In plain words** (1–3 sentences, no jargon, an everyday analogy),
**How it works** (the mechanism in simple steps, terms explained or linked to
the glossary), **For developers** (crates, modules as repo-relative links,
key types and events, how to extend it).

Everywhere: short paragraphs; relative links; Mermaid diagrams with node ids
without spaces, special-character labels in double quotes, no colors; keep a
page under about 380 lines and split by topic rather than growing it.

## 6. The website

The docs are also a website,
[arshadbarves.github.io/z-engine](https://arshadbarves.github.io/z-engine/).
It publishes the pages in place: everything under `docs/`, plus the root
`README.md`, `AGENTS.md` and `CHANGELOG.md`. [website/](../website/) holds
only the VitePress site around them (config, theme, sidebar and the home
page). The [docs-site workflow](../.github/workflows/docs-site.yml) builds
and deploys it on every push to `release` that changes a page or the site.

- Preview it with `npm ci --prefix website` (once), then
  `npm run dev --prefix website`.
- List every new page in
  [website/.vitepress/sidebar.ts](../website/.vitepress/sidebar.ts):
  `npm test --prefix website` and `npm run build --prefix website` both
  fail while a page under `docs/` is missing from it.
- Keep writing relative links. On the site, a link to a source file or
  folder (anything that is not a published page) becomes a link to it on
  GitHub's `release` branch.

## 7. Checklist before you finish a change

1. List what changed: `git status --short` and `git diff --stat`.
2. Map every change to documents with the table in section 3.
3. Confirm each fact in the code (section 4), then update the pages and
   the `[Unreleased]` changelog entry.
4. Search for stale mentions of anything renamed or removed, e.g.
   `grep -rn "<old name>" README.md AGENTS.md docs .claude`.
5. Run `python3 scripts/check_docs_links.py`, `npm test --prefix website`
   and `npm run build --prefix website` (CI runs all three).
6. Say in your summary which documents you updated, or why none needed to.
