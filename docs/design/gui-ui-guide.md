# Z Engine GUI — UI Guide (Svelte 5)

Status: **canonical** · Frontend is Svelte 5 + Bits UI + Vite, hosted by Tauri 2.
Read this before adding or changing any file under `crates/z-engine-gui/ui`.

This guide exists so humans and LLMs produce the same kind of code: small
files, one responsibility, reusable primitives, no ad-hoc overlays.

## 1. Why this stack

The v0.1 React UI grew a 1 000-line event god-module, hand-rolled popovers /
dialogs / menus, and `useSyncExternalStore` boilerplate in every screen.
That is the clutter this rewrite removes.

| Choice | Why |
|---|---|
| **Svelte 5 (Vite, no SvelteKit)** | Single-window Tauri SPA. Kit routing/SSR adds nothing and fights the webview. |
| **Runes + thin store bind** | Components read reactive values. Domain/runtime stays plain TypeScript so vitest does not need a browser. |
| **Bits UI (headless)** | Accessible Dialog, Menu, Select, Tabs, Combobox, Popover, Tooltip. We own the look. |
| **No Tailwind / no shadcn-svelte** | The Linear warm-neutral / Arc island look is already in CSS tokens. A second design system would fight it. |
| **Hugeicons (core paths)** | Same icon set as before; rendered by our `Icon` primitive, not a React wrapper. |

Do not add a component library that ships its own theme.

## 2. Dependency direction

```
.svelte screens  →  $lib/ui primitives (Bits UI wrappers)
                 →  $lib/runtime (transcript / session / events)
                 →  $lib/domain  (pure helpers + tests)
                 →  $lib/commands.ts   (ONLY Tauri invoke)
```

- Screens never call `invoke()` or `@tauri-apps/api/core`.
- Event listening lives in `lib/runtime/listen.ts` only.
- Bits UI is imported only inside `lib/ui/*`. Feature components use our
  wrappers (`Dialog`, `Menu`, `Select`, …).
- Rust/core is unchanged. The IPC contract in `commands.ts` is the seam.

## 3. Directory map

```
crates/z-engine-gui/ui/src/
├── main.ts                 # mount + platform class + global CSS
├── App.svelte              # composition root (wiring only)
├── index.css               # tokens, reset, and shared app/chat layout
├── chrome.css              # chrome and supporting shell layout
├── settings.css            # settings/full-screen inspector layout
├── splash.css              # splash screen
├── lib/
│   ├── commands.ts         # typed invoke wrappers — IPC boundary
│   ├── types.ts            # Msg, Toast, SessionEntry, shared types
│   ├── domain/             # pure functions (sessionList, diffParse, …)
│   ├── runtime/            # agent event loop + session park/replay
│   │   ├── state.ts        # mutable session fields + store objects
│   │   ├── mutations.ts    # push / update / trim / resolveApproval
│   │   ├── dispatch.ts     # handleEvent switch
│   │   ├── session.ts      # activate / park / queue drain
│   │   ├── replay.ts       # JSONL → cards
│   │   ├── listen.ts       # initEvents (Tauri listen, once)
│   │   └── index.ts        # re-exports (lib/events.ts stays a façade)
│   ├── stores/             # app-level stores (config, workspace, update, ui)
│   ├── ui/                 # Bits UI kit + Icon + Button + SegmentedChoice
│   │                       # + presence + copyFeedback
│   └── svelte/             # bindStore() — store → rune
└── components/
    ├── chrome/             # TopBar, WindowControls, Splash, Logo
    ├── sidebar/
    ├── chat/               # timeline, cards, composer
    │   └── primitives/     # native Svelte chat presentation catalog
    ├── settings/
    └── overlays/           # palette, diff, worktree, shell
```

File budget (same as the repo): target ≤300 lines, hard cap 400. Split by
responsibility, never by percentage.

## 4. Patterns

### 4.1 Component shape

```svelte
<script lang="ts">
  import { Button } from "$lib/ui/Button.svelte";
  import { bindStore } from "$lib/svelte/bind.svelte";
  import { transcriptStore } from "$lib/runtime";

  type Props = { pending?: boolean };
  let { pending = false }: Props = $props();

  const messages = bindStore(transcriptStore);

  function onSend() {
    /* call a function from lib/runtime or lib/commands — no invoke() */
  }
</script>

<section class="composer">
  <Button variant="accent" disabled={pending} onclick={onSend}>Send</Button>
</section>
```

- `$props()` / `$state` / `$derived` / `$effect` only. No Svelte 4 `export let`.
- Props are a typed object. Events that bubble use callback props
  (`onClose`, `onApprove`), not `createEventDispatcher`.
- One visual thing per file. If a file names a card, it renders that card.

### 4.2 Stores

Runtime stores keep the battle-tested `{ subscribe, getSnapshot }` shape so
`src/lib/runtime/*.test.ts` stays node-vitest. Screens bind them:

```ts
const busy = bindStore(busyStore); // busy.current
```

App chrome state (palette open, settings open, sidebar) lives in
`lib/stores/ui.svelte.ts` as runes — it is UI-only and is not unit-tested
through the event loop.

Never put Tauri listeners or `invoke` inside a store except `listen.ts`,
`commands.ts`, and the workspace/update stores that already wrap a single
command.

### 4.3 Bits UI — use the kit, not the package

```svelte
<!-- YES -->
<Dialog.Root bind:open>
  <Dialog.Content title="Worktree">…</Dialog.Content>
</Dialog.Root>

<!-- NO — do not import bits-ui from a feature component -->
<script>
  import { Dialog } from "bits-ui";
</script>
```

Wrappers apply our tokens (`--surface`, `--radius-m`, `--shadow-floating`)
and the existing class names (`.dialog`, `.menu`, `.select`, …). When Bits
UI’s API moves, only `lib/ui/` changes.

Use Bits primitives for:

| Need | Kit |
|---|---|
| Modal / settings / worktree | `Dialog` |
| Confirm destructive | `Dialog` (alert variant) |
| Context menu, overflow | `Menu` |
| Mode / model / effort | `Select` |
| Settings sections | `Tabs` |
| ⌘K palette, @file, slash | `Combobox` |
| Composer extras | `Popover` |
| Icon button labels | `Tooltip` |

Do not invent another `position: fixed` overlay with a backdrop `div`
unless Bits has no primitive for it (the boot splash is the exception).

### 4.4 Native chat primitives

- `components/chat/primitives/` is the native Svelte presentation catalog:
  conversation turns, assistant messages, message actions, process activity,
  reasoning, evidence, status, and task-report summaries.
- `ConversationTurn` selects the presentation for each timeline block.
  Assistant reasoning and tool calls are grouped into one `ToolActivityGroup`
  process disclosure; its tabs separate files, searches, terminal activity,
  and reasoning without adding parallel transcript cards.
- Task reports have three presentation modes. **Quiet** is the default and
  shows the result with details disclosed on demand. **Compact** adds a short
  summary and check count while details remain collapsed. **Detailed** keeps
  checks and evidence inline.
- The Appearance tab persists this choice as `task_report_view` through the
  typed settings command and reconciles the saved value after an update.
- These are local Svelte components, not an assistant-chat framework
  dependency.

### 4.5 CSS and visual hierarchy

- Tokens, reset, shared app layout, and chat styles live in `index.css`.
  `main.ts` also imports `chrome.css`. Settings and prompt-inspector screens
  import `settings.css`; splash-only rules live in `splash.css`.
- Prefer the **existing class names** (`.app`, `.transcript`, `.composer`,
  `.sidebar`, …).
- Keep the hierarchy low-clutter: stage → floating island → content column →
  quiet nested metadata. Prefer spacing and type weight before adding another
  border, badge, or filled panel.
- Reuse semantic tokens: `--surface-raised` for elevated content,
  `--surface-input` for inputs, `--surface-quiet` for nested metadata,
  `--hover-*`/`--border-hover` for interaction, and `--tone-*` only for
  meaningful status. Keep transcript and composer aligned with
  `--measure-chat`, `--gutter-chat`, and `--column-chat`.
- Scoped `<style>` in a component is allowed for one-off layout that will
  never be reused. Shared look belongs in the existing root CSS file for that
  area; do not create a second styles tree.
- No inline style objects except chart/canvas geometry.

### 4.6 Icons

```svelte
<script>
  import { Icon, Plus } from "$lib/ui/icons";
</script>
<Icon icon={Plus} size={16} />
```

Add a new icon in `lib/ui/icons.ts` only. Do not import
`@hugeicons/core-free-icons` from a screen.

### 4.7 Full-screen overlay screens (Settings, Prompt Inspector, full-window views)

Full-window overlays must match the home screen's stage + floating island container architecture:

1. **Stage & Window Layout**:
   - Outer stage uses `--bg` (`#000`) with smooth `sheet-in` / `sheet-out` transitions.
   - Draggable 40px TopBar (`.app-topbar`) with `data-tauri-drag-region`.
   - macOS traffic lights clearance (`padding-left: 88px` on `html.plat-mac`).
   - Back button on top-left (`<button class="icon-btn" title="Back (Esc)" onclick={onClose}><Icon icon={ChevronLeft} size={15} /></button>`).
   - Breadcrumbs (`Root / ActiveSection`).
   - Right-side `<WindowControlsMaybe />` for Windows/Linux.

2. **Floating Islands**:
   - Body has `padding: 0 6px 6px 6px` and `gap: 6px` (identical to `.app-body`).
   - Left Navigation Island (`.settings-nav-island`): `background: var(--bg-sidebar); border: 1px solid var(--border); border-radius: var(--radius-m); box-shadow: 0 4px 20px rgba(0, 0, 0, 0.25);`.
   - Right Canvas Island (`.settings-canvas-pane`): `background: var(--bg); border: 1px solid var(--border); border-radius: var(--radius-m); box-shadow: 0 4px 20px rgba(0, 0, 0, 0.25);`.
   - Top of canvas has a 46px header bar (`.settings-pane-head`) matching `.chat-head`.
   - Content body is centered with `max-width: 720px` (or `920px` for wide inspector/diff views).

3. **Navigation & Shortcuts**:
   - Do not display explicit `<kbd>Esc</kbd>` close badges.
   - Always bind a keyboard `keydown` listener for `Escape` to trigger `onClose()`.
   - Never create flat edge-to-edge sheets without the floating container structure.

## 5. How to add things

| Adding… | Do this |
|---|---|
| A screen control | New `components/<area>/<Name>.svelte`. Use kit primitives. Keep ≤300 lines. |
| A Bits wrapper | New file in `lib/ui/`. Re-export from `lib/ui/index.ts`. |
| An IPC command | Rust `commands/<domain>.rs` + `generate_handler!` + wrapper in `lib/commands.ts`. Then call the wrapper. |
| An agent event | `handleEvent` in `lib/runtime/dispatch.ts` + type in `lib/types.ts`. Never listen in a component. |
| A pure helper | `lib/domain/<name>.ts` + sibling `*.test.ts`. |
| A CSS token | `--name` in `index.css`. Use it; do not hard-code hex in components. |

## 6. Testing

- Domain + runtime: vitest, node environment, next to the file
  (`sessionList.test.ts`, `dispatch` via `events.test.ts`).
- Do not mount Svelte in unit tests unless the logic cannot be extracted.
- `pnpm test` and `pnpm check` must stay green. `pnpm build` is the Tauri
  frontend compile.

## 7. What not to do

- Do not reintroduce React, `useSyncExternalStore`, or `.tsx`.
- Do not add SvelteKit, Tailwind, shadcn-svelte, or Melt UI.
- Do not grow `App.svelte` past wiring (stores → screens). Put handlers in
  `lib/stores/app-actions.ts` or the relevant domain module.
- Do not call `listen("appEvent")` a second time. `initEvents()` is
  one-shot by design.
- Do not copy-paste a dialog/menu/select. Extend the kit.
- Do not put prompt text, Rust types, or provider IDs in the UI. The
  frontend is presentation-only.

## 8. Keyboard map (must keep)

| Shortcut | Action |
|---|---|
| ⌘/Ctrl+K | Command palette |
| ⌘/Ctrl+N | New task |
| ⌘/Ctrl+B | Toggle sidebar |
| Enter | Send (composer) |
| Shift+Enter | Newline |
| Esc | Close overlay / abort turn (existing behavior) |

## 9. Visual language (do not restyle casually)

- Black `--bg` stage, floating islands, 13px system UI font.
- Accent is white; status uses `--ok`, `--err`, `--warn`, and `--tone-*`.
- Radius 8 / 14 / 20. Quiet hairline borders at 8–14% white.
- Overlay title bar; macOS traffic lights stay system-drawn.

Changing the palette is a design change, not a drive-by cleanup.
