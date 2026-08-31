<script lang="ts">
  import type { PaletteItem } from "$lib/paletteTypes";
  import { sessionLabel } from "$lib/sessionList";
  import type { SessionEntry } from "$lib/util";
  import { wsBasename } from "$lib/workspaces";
  import { FolderGit2, Icon, MessageSquare, Search, X } from "$lib/ui/icons";

  type Props = {
    isClosing?: boolean;
    onClose: () => void;
    sessions: SessionEntry[];
    workspaces: string[];
    activeWorkspace: string | null;
    actions: PaletteItem[];
    onOpenSession: (path: string, projectRoot?: string | null) => void;
    onActivateWorkspace: (root: string) => void;
  };

  let {
    isClosing = false,
    onClose,
    sessions,
    workspaces,
    activeWorkspace,
    actions,
    onOpenSession,
    onActivateWorkspace,
  }: Props = $props();

  let query = $state("");
  let sel = $state(0);
  let listEl: HTMLDivElement | undefined = $state();

  /** Subsequence fuzzy match: every query char must appear in order.
   * Score = total span of matched positions (lower is better). */
  function fuzzyScore(qRaw: string, item: PaletteItem): number | null {
    const q = qRaw.trim().toLowerCase();
    if (!q) return Number.POSITIVE_INFINITY;
    const hay = `${item.label} ${item.keywords} ${item.group ?? ""}`.toLowerCase();
    let hi = 0;
    let prev = -1;
    let first = -1;
    for (const ch of q) {
      const idx = hay.indexOf(ch, prev + 1);
      if (idx === -1) return null;
      if (first === -1) first = idx;
      if (idx === prev + 1) {
        if (hi - first > 64) return null;
      }
      prev = idx;
      hi = idx;
    }
    return hi - Math.max(0, first - 8);
  }

  const items = $derived.by(() => {
    const sessionItems: PaletteItem[] = sessions.slice(0, 8).map((s) => ({
      label: sessionLabel(s.firstUserMsg),
      hint: s.projectRoot ? wsBasename(s.projectRoot) : "Chat",
      keywords: `session chat ${s.ulid} ${s.projectRoot ?? ""}`,
      group: "Recent Chats",
      icon: MessageSquare,
      run: () => onOpenSession(s.path, s.projectRoot),
    }));

    const wsItems: PaletteItem[] = workspaces.map((root) => ({
      label: wsBasename(root),
      hint: root === activeWorkspace ? "Active Workspace" : "Switch Workspace",
      keywords: `workspace project folder ${root}`,
      group: "Workspaces",
      icon: FolderGit2,
      run: () => onActivateWorkspace(root),
    }));

    return [...actions, ...wsItems, ...sessionItems]
      .map((item) => ({ item, score: fuzzyScore(query, item) }))
      .filter(({ score }) => score !== null)
      .sort((a, b) => (a.score as number) - (b.score as number))
      .map(({ item }) => item);
  });

  const selIndex = $derived(Math.min(sel, Math.max(0, items.length - 1)));

  const groups = $derived.by(() => {
    const out: { name: string | undefined; items: { item: PaletteItem; index: number }[] }[] = [];
    let i = 0;
    for (const item of items) {
      const entry = { item, index: i++ };
      const g = out.find((x) => x.name === item.group);
      if (g) g.items.push(entry);
      else out.push({ name: item.group, items: [entry] });
    }
    return out;
  });

  $effect(() => {
    void selIndex;
    const activeEl = listEl?.querySelector(".palette-row.is-selected");
    if (activeEl) activeEl.scrollIntoView({ block: "nearest" });
  });

  function run(i: number) {
    const item = items[i];
    if (item) {
      onClose();
      item.run();
    }
  }

  function onQueryInput() {
    sel = 0;
  }

  function onInputKey(e: KeyboardEvent) {
    const n = Math.max(1, items.length);
    if (e.key === "ArrowDown") {
      e.preventDefault();
      sel = (sel + 1) % n;
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      sel = (sel - 1 + items.length) % n;
    } else if (e.key === "Enter") {
      e.preventDefault();
      run(selIndex);
    } else if (e.key === "Escape") {
      e.preventDefault();
      onClose();
    }
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="palette-backdrop{isClosing ? ' is-closing' : ''}" onmousedown={onClose}>
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="palette-spotlight{isClosing ? ' is-closing' : ''}" onmousedown={(e) => e.stopPropagation()}>
    <div class="palette-header">
      <div class="palette-search-icon-box">
        <Icon icon={Search} size={15} strokeWidth={2} />
      </div>
      <!-- svelte-ignore a11y_autofocus -->
      <input
        autofocus
        class="palette-input"
        bind:value={query}
        oninput={onQueryInput}
        onkeydown={onInputKey}
        placeholder="Type a command or search actions, chats, workspaces…"
        spellcheck={false}
      />
      {#if query}
        <button
          type="button"
          class="palette-clear-btn"
          title="Clear search"
          onclick={() => {
            query = "";
            sel = 0;
          }}
        >
          <Icon icon={X} size={13} strokeWidth={2} />
        </button>
      {:else}
        <span class="palette-count-chip">{items.length}</span>
      {/if}
    </div>

    <div class="palette-body" bind:this={listEl}>
      {#if items.length === 0}
        <div class="palette-empty-state">
          <div class="palette-empty-icon">
            <Icon icon={Search} size={22} strokeWidth={1.5} />
          </div>
          <span class="palette-empty-title">No matching results</span>
          <span class="palette-empty-sub">
            Try typing an action name, workspace, or session keyword.
          </span>
        </div>
      {:else}
        {#each groups as g (g.name ?? "_general")}
          <div class="palette-section">
            {#if g.name}
              <div class="palette-section-title">{g.name}</div>
            {/if}
            {#each g.items as { item, index } (`${item.label}-${index}`)}
              {@const isSelected = index === selIndex}
              <button
                type="button"
                class="palette-row{isSelected ? ' is-selected' : ''}"
                onmouseenter={() => (sel = index)}
                onclick={() => run(index)}
              >
                <div class="palette-row-icon-box">
                  <Icon icon={item.icon ?? Search} size={14} strokeWidth={1.8} />
                </div>
                <div class="palette-row-content">
                  <span class="palette-row-label">{item.label}</span>
                  {#if item.hint}
                    <span class="palette-row-hint">{item.hint}</span>
                  {/if}
                </div>
                <div class="palette-row-trailing">
                  {#if item.shortcut}
                    <kbd class="palette-shortcut-badge">{item.shortcut}</kbd>
                  {:else if isSelected}
                    <kbd class="palette-shortcut-badge">↵</kbd>
                  {/if}
                </div>
              </button>
            {/each}
          </div>
        {/each}
      {/if}
    </div>

    <div class="palette-footer">
      <div class="palette-footer-shortcuts">
        <span class="footer-shortcut-item">
          <kbd>↑↓</kbd> Navigate
        </span>
        <span class="footer-shortcut-item">
          <kbd>↵</kbd> Execute
        </span>
        <span class="footer-shortcut-item">
          <kbd>Esc</kbd> Close
        </span>
      </div>
      <div class="palette-footer-brand">
        <span class="footer-brand-text">Z Engine Spotlight</span>
      </div>
    </div>
  </div>
</div>
