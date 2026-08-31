# Svelte 5 GUI rewrite — design

Date: 2026-08-31
Status: approved (Option A — full rewrite)
Owner: z-engine-gui frontend

## Problem

The React 19 GUI works, but it is not a production-grade frontend:

- `lib/events.ts` is 1 065 lines of session runtime, stores, dispatch, and replay.
- Dialogs, menus, selects, tabs, and the command palette are hand-rolled.
- Every screen repeats `useSyncExternalStore` plumbing.
- `App.tsx` owns session CRUD, shortcuts, overlays, and layout.
- AGENTS.md still documents a React tree, so future agents will keep growing it.

The original GUI design (`docs/design/gui-v0.1.md`) was Svelte 5. React was a
detour. We go back, on purpose, with a kit.

## Decision

**Option A — full rewrite** of `crates/z-engine-gui/ui` to:

- Svelte 5 + Vite (no SvelteKit)
- Bits UI for headless overlays / lists / tabs
- A small in-repo primitive kit (`lib/ui`)
- Split runtime modules with a stable public façade (`lib/events.ts`)
- Canonical guide: `docs/design/gui-ui-guide.md`

Out of scope: Rust/core, TUI, visual redesign, Tailwind, shadcn-svelte.

## Architecture

Unchanged IPC: `commands.ts` ↔ Tauri `#[tauri::command]` ↔ `AgentHandle`.
Unchanged events: `listen("appEvent")` in one place.

```
App.svelte (wiring)
  chrome / sidebar / chat / settings / overlays
       │
       ▼
  lib/ui  (Bits UI wrappers + Icon + Button)
       │
  lib/runtime + lib/stores + lib/domain
       │
  lib/commands.ts
```

Runtime keeps `{ subscribe, getSnapshot }` so existing vitest files stay
node-side. Svelte screens bind through `bindStore()`.

## Libraries (keep the set small)

| Package | Role |
|---|---|
| `svelte` | UI |
| `bits-ui` | Headless primitives |
| `@hugeicons/core-free-icons` | Icon path data |
| `svelte-exmarkdown` + `remark-gfm` | Assistant markdown |
| `highlight.js` | Code blocks (replaces react-syntax-highlighter) |
| `mermaid` | Diagrams (existing) |
| `@tauri-apps/*` | IPC / dialog / window |

Removed: `react`, `react-dom`, `@hugeicons/react`, `react-markdown`,
`react-syntax-highlighter`, `@vitejs/plugin-react`.

## Compatibility

- CSS class names stay (`.app`, `.transcript`, `.composer`, …).
- Session park/replay, approvals, queue flush, hydrate lock — same behavior.
- Keyboard map unchanged.
- Tests that import `./events` keep compiling against the façade.

## Acceptance

- `pnpm check` / `pnpm test` / `pnpm build` green in `ui/`
- App shell: splash → sidebar + transcript + composer
- Overlays: palette, settings, diff, worktree use Bits `Dialog` / `Combobox`
- AGENTS.md documents the Svelte tree and points at the UI guide
- No `.tsx` left under `ui/src`
