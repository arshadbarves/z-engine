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
├── app.css                 # tokens + reset only
├── styles/                 # layout / chat / chrome / overlays (no logic)
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
│   ├── ui/                 # Bits UI kit + Icon + Button + presence
│   └── svelte/             # bindStore() — store → rune
└── components/
    ├── chrome/             # TopBar, WindowControls, Splash, Logo
    ├── sidebar/
    ├── chat/               # timeline, cards, composer
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

### 4.4 CSS

- Tokens live in `app.css` (`--bg`, `--accent`, `--radius-*`, …).
- Feature CSS is in `styles/*.css`, imported once from `main.ts`.
- Prefer the **existing class names** (`.app`, `.transcript`, `.composer`,
  `.sidebar`, …). The visual language is Linear warm-neutral + Arc islands.
- Scoped `<style>` in a component is allowed for one-off layout that will
  never be reused. Shared look goes in `styles/`.
- No inline style objects except chart/canvas geometry.

### 4.5 Icons

```svelte
<script>
  import { Icon, Plus } from "$lib/ui/icons";
</script>
<Icon icon={Plus} size={16} />
```

Add a new icon in `lib/ui/icons.ts` only. Do not import
`@hugeicons/core-free-icons` from a screen.

## 5. How to add things

| Adding… | Do this |
|---|---|
| A screen control | New `components/<area>/<Name>.svelte`. Use kit primitives. Keep ≤300 lines. |
| A Bits wrapper | New file in `lib/ui/`. Re-export from `lib/ui/index.ts`. |
| An IPC command | Rust `commands/<domain>.rs` + `generate_handler!` + wrapper in `lib/commands.ts`. Then call the wrapper. |
| An agent event | `handleEvent` in `lib/runtime/dispatch.ts` + type in `lib/types.ts`. Never listen in a component. |
| A pure helper | `lib/domain/<name>.ts` + sibling `*.test.ts`. |
| A CSS token | `--name` in `app.css`. Use it; do not hard-code hex in components. |

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

- Background `#141416` stage, floating islands, 13px system UI font.
- Accent / ok `#4ebd8f`. Error `#d96568`. Warn `#d69e48`.
- Radius 6 / 10 / 14. Quiet hairline borders at 6–10% white.
- Overlay title bar; macOS traffic lights stay system-drawn.

Changing the palette is a design change, not a drive-by cleanup.
