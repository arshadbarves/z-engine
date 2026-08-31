# Diff Panel UX Redesign (Approach B)

Status: **approved for implementation** · Date: 2026-08-31  
Surface: `crates/z-engine-gui/ui` Diff review overlay  
Reference: Cursor Review (file tree + one-file viewer) — layout/calm chrome only, not a product clone.

## Problem

The current Diff panel is functionally correct (Chat-scoped by default, Git secondary) but visually busy:

- Status tabs (All / Mod / Add / Del) and `+A` / `~M` / `-D` badges compete with the file names
- Header packs title, scope toggle, nav, refresh, and close without hierarchy
- File list is on the left; the screenshot pattern puts the **diff first** (left) and the **tree second** (right)

Users need a calmer review surface that still answers: “what did this chat change?”

## Goals

1. Less clutter, clearer hierarchy — scan the tree, read one file.
2. Match the useful bits of the reference: **diff left**, **collapsible folder tree right**, quiet status dots, `+N/−M` stats.
3. Keep Chat vs Git scope and existing `DiffView` rendering.
4. Stay inside AGENTS.md budgets (≤300 target / 400 hard) by splitting components.

## Non-goals

- Commit / push / branch picker / PR comments / multi-file accept UI
- Cloning Cursor Review chrome wholesale
- Persisting checkpoints across resume (unchanged limitation)

## Layout

```
┌─ Session changes · N files · +a −d · [Chat|Git] · ‹ › · ✕ ─┐
│ ℹ Showing one file at a time                          ‹ ›  │
├──────────────────────────────┬─────────────────────────────┤
│ DiffView (selected file)     │ Filter files…               │
│ path · +50 −0                │ ▾ crates/                   │
│ green / red unified lines    │   ▾ z-engine-gui/ui/…/chat/ │
│                              │     ● ModePicker.svelte +50 │
│                              │     ○ ToolCard.svelte       │
└──────────────────────────────┴─────────────────────────────┘
```

- Docked resizable panel (existing behavior); resize handle stays on the chat-facing edge.
- **Left:** one-file `DiffView` (existing).
- **Right:** collapsible folder tree + filter.

## Header

| Element | Behavior |
|---|---|
| Title | `Session changes` when scope=Chat; `Git changes` when scope=Git |
| Counts | `N files` + aggregate `+a −d` from cached/parsed diffs when available |
| Scope | Quiet segmented **Chat \| Git** control (unchanged semantics) |
| Nav | Prev / next file among filtered leaf order |
| Close | Keep; refresh as a small icon (no primary emphasis) |

## Hint bar

Shown only when the filtered file set has **≥ 2** leaves:

> Showing one file at a time

Right side: prev / next. Single-file sessions omit the bar.

## File tree (right)

- Build a collapsible tree from workspace-relative paths.
- Folders: chevron + name; default **expanded** (or expand ancestors of the selection).
- Leaves: file icon + basename; **status color dot** (modified / added / deleted) — no `+A/~M/-D` text badges.
- When a file’s diff is in cache, show trailing `+N −M` (from `parseGitDiff`).
- Filter box filters leaves by path substring; folders with no matching leaves collapse out of view.
- **Remove** All / Mod / Add / Del tabs.
- Selected leaf: subtle highlight (border/background), not a heavy pill.

## Diff pane (left)

- Keep `DiffView` as-is (path bar, copy, `+N/−M` pill, colored rows).
- Empty / loading / error states stay short and plain.

## Data / scope (unchanged)

| Scope | File list | Diff baseline |
|---|---|---|
| Chat (default) | `list_session_changed_files` | Checkpoint pre-image → disk |
| Git | `list_changed_files` | `git diff HEAD` |

No new Tauri commands required for the UX pass. Aggregate `+a/−d` is derived client-side as diffs enter the cache (lazy: update header as files are opened / optionally prefetch selected + neighbors).

## Component split

| File | Responsibility |
|---|---|
| `DiffPanel.svelte` | Shell: header, hint bar, scope, resize, wiring |
| `DiffFileTree.svelte` | Tree render + filter + selection |
| `$lib/domain/diffTree.ts` | Pure path → tree builder (+ vitest) |
| `DiffView.svelte` | Unchanged unless tiny file-bar polish |
| `index.css` | Tree + slim header styles; delete unused tab chrome if orphaned |

## Interaction details

- Opening the panel selects the first filtered leaf and loads its diff.
- Keyboard: `[` / `]` (or existing prev/next) walks leaves in tree order.
- Scope switch clears selection cache keys and reloads the list.
- Folder collapse state is component-local (not persisted).

## Success criteria

1. Default open shows Chat-scoped files only; unrelated dirty files absent.
2. Diff is on the left; tree on the right with collapsible folders.
3. No status tabs / text status badges; dots + optional `+N/−M` only.
4. Header shows aggregate stats once at least one diff is parsed.
5. Hint bar appears iff ≥2 files.
6. File budget respected after split; `svelte-check` clean for touched files.

## Out of follow-up (optional later)

- Prefetch all session diffs for accurate header totals on open
- Transcript-path fallback after resume
- Persist folder collapse state
