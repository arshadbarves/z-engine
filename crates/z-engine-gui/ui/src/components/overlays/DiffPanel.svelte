<script lang="ts">
  import { onMount } from "svelte";
  import {
    diffForFile,
    listChangedFiles,
    listSessionChangedFiles,
    sessionDiffForFile,
    type ChangedFile,
  } from "$lib/commands";
  import { flattenDiffTree, buildDiffTree, filterDiffTree } from "$lib/domain/diffTree";
  import { sessionStore } from "$lib/runtime/state";
  import { bindStore } from "$lib/svelte/bind.svelte";
  import Icon, {
    ChevronLeft,
    ChevronRight,
    FileCode,
    GitCompare,
    Info,
    RefreshCw,
    X,
  } from "$lib/ui/icons";
  import DiffFileTree from "./DiffFileTree.svelte";
  import DiffView from "./DiffView.svelte";

  type Scope = "session" | "git";
  type Props = { isClosing?: boolean; onClose: () => void };
  let { isClosing = false, onClose }: Props = $props();

  const sessionId = bindStore(sessionStore);
  let scope = $state<Scope>("session");
  let files = $state<ChangedFile[] | null>(null);
  let error = $state<string | null>(null);
  let selectedPath = $state<string | null>(null);
  let diffCache = $state<Record<string, string>>({});
  let loadingDiff = $state(false);
  let refreshing = $state(false);
  let searchQuery = $state("");
  let width = $state(720);
  let isResizing = $state(false);
  let loadGen = 0;

  onMount(() => {
    let active = true;
    void refresh().finally(() => {
      if (!active) return;
    });
    return () => {
      active = false;
    };
  });

  async function loadFiles(): Promise<ChangedFile[]> {
    if (scope === "session") {
      return listSessionChangedFiles(sessionId.current || null);
    }
    return listChangedFiles();
  }

  async function loadDiff(path: string): Promise<string> {
    if (scope === "session") {
      return sessionDiffForFile(path, sessionId.current || null);
    }
    return diffForFile(path);
  }

  async function refresh() {
    const gen = ++loadGen;
    refreshing = true;
    try {
      const next = await loadFiles();
      if (gen !== loadGen) return;
      files = next;
      error = null;
      if (next.length > 0 && (!selectedPath || !next.some((f) => f.path === selectedPath))) {
        if (next[0]) void selectFile(next[0].path);
      } else if (next.length === 0) {
        selectedPath = null;
      }
    } catch (e) {
      if (gen !== loadGen) return;
      error = String(e);
      files = [];
    } finally {
      if (gen === loadGen) refreshing = false;
    }
  }

  async function setScope(next: Scope) {
    if (scope === next) return;
    scope = next;
    selectedPath = null;
    diffCache = {};
    searchQuery = "";
    files = null;
    await refresh();
  }

  async function selectFile(path: string) {
    selectedPath = path;
    const cacheKey = `${scope}:${path}`;
    if (diffCache[cacheKey] !== undefined) return;
    loadingDiff = true;
    try {
      diffCache = { ...diffCache, [cacheKey]: await loadDiff(path) };
    } catch (e) {
      diffCache = { ...diffCache, [cacheKey]: `(no diff available: ${String(e)})` };
    } finally {
      loadingDiff = false;
    }
  }

  const leafOrder = $derived.by(() => {
    if (!files) return [] as string[];
    return flattenDiffTree(filterDiffTree(buildDiffTree(files), searchQuery));
  });

  function selectAdjacent(dir: -1 | 1) {
    const list = leafOrder;
    if (list.length === 0) return;
    const idx = list.findIndex((p) => p === selectedPath);
    const nextIdx = idx === -1 ? 0 : Math.max(0, Math.min(list.length - 1, idx + dir));
    const next = list[nextIdx];
    if (next) void selectFile(next);
  }

  function onResizePointerDown(e: PointerEvent) {
    e.preventDefault();
    isResizing = true;
    const startX = e.clientX;
    const startWidth = width;
    function onPointerMove(ev: PointerEvent) {
      const maxW = Math.max(480, window.innerWidth - 320);
      width = Math.max(380, Math.min(maxW, startWidth + (startX - ev.clientX)));
    }
    function onPointerUp() {
      isResizing = false;
      window.removeEventListener("pointermove", onPointerMove);
      window.removeEventListener("pointerup", onPointerUp);
    }
    window.addEventListener("pointermove", onPointerMove);
    window.addEventListener("pointerup", onPointerUp);
  }

  const summary = $derived.by(() => {
    let created = 0;
    let updated = 0;
    let deleted = 0;
    let added = 0;
    let removed = 0;
    for (const f of files ?? []) {
      if (f.status === "added") created++;
      else if (f.status === "deleted") deleted++;
      else updated++;
      added += f.added ?? 0;
      removed += f.deleted ?? 0;
    }
    return { created, updated, deleted, added, removed, files: files?.length ?? 0 };
  });

  const activeDiff = $derived(
    selectedPath ? diffCache[`${scope}:${selectedPath}`] ?? null : null,
  );
  const title = $derived(scope === "session" ? "Review" : "Review · Git");
  const emptyLabel = $derived(
    scope === "session" ? "No changes in this chat" : "Working tree clean",
  );
  const showHint = $derived((files?.length ?? 0) >= 2);
</script>

<aside
  class={`diff-panel${isClosing ? " is-closing" : ""}${isResizing ? " is-resizing" : ""}`}
  style={`width: ${width}px; --diff-w: ${width}px;`}
>
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="diff-resize-handle" onpointerdown={onResizePointerDown} role="separator" aria-orientation="vertical" title="Drag to resize review pane"></div>

  <div class="diff-head-pro">
    <div class="diff-head-title-wrap">
      <Icon icon={GitCompare} size={14} class="diff-title-icon" />
      <span class="diff-title-pro">{title}</span>
      {#if summary.files > 0}
        <span class="diff-count-badge-pro">{summary.files}</span>
        <span class="diff-status-summary">
          {#if summary.created > 0}<span class="added">{summary.created} created</span>{/if}
          {#if summary.updated > 0}<span class="modified">{summary.updated} updated</span>{/if}
          {#if summary.deleted > 0}<span class="deleted">{summary.deleted} deleted</span>{/if}
        </span>
        {#if summary.added > 0 || summary.removed > 0}
          <span class="diff-agg-stats">
            {#if summary.added > 0}<span class="add">+{summary.added}</span>{/if}
            {#if summary.removed > 0}<span class="del">−{summary.removed}</span>{/if}
          </span>
        {/if}
      {/if}
    </div>
    <div class="diff-head-actions-pro">
      <div class="diff-scope-toggle" role="group" aria-label="Diff scope">
        <button type="button" class={`diff-scope-btn${scope === "session" ? " active" : ""}`} onclick={() => void setScope("session")} title="Only files this chat edited">Chat</button>
        <button type="button" class={`diff-scope-btn${scope === "git" ? " active" : ""}`} onclick={() => void setScope("git")} title="All uncommitted git changes vs HEAD">Git</button>
      </div>
      <button type="button" class={`icon-btn-mini${refreshing ? " spinning" : ""}`} title="Refresh" onclick={() => void refresh()}>
        <Icon icon={RefreshCw} size={12} />
      </button>
      <button type="button" class="icon-btn-mini" title="Close review pane (Esc)" onclick={onClose}>
        <Icon icon={X} size={13} />
      </button>
    </div>
  </div>

  {#if showHint}
    <div class="diff-hint-bar">
      <div class="diff-hint-left">
        <Icon icon={Info} size={12} />
        <span>Showing one file at a time</span>
      </div>
      <div class="diff-nav-arrows">
        <button type="button" class="icon-btn-mini" title="Previous file ([)" onclick={() => selectAdjacent(-1)}>
          <Icon icon={ChevronLeft} size={12} />
        </button>
        <button type="button" class="icon-btn-mini" title="Next file (])" onclick={() => selectAdjacent(1)}>
          <Icon icon={ChevronRight} size={12} />
        </button>
      </div>
    </div>
  {/if}

  <div class="diff-stage-layout diff-stage-review">
    <div class="diff-viewer-pane">
      {#if selectedPath}
        {#if loadingDiff && !activeDiff}
          <div class="diff-viewer-empty">Loading diff for {selectedPath}…</div>
        {:else if activeDiff}
          <DiffView text={activeDiff} filePath={selectedPath} />
        {:else}
          <div class="diff-viewer-empty">No diff content available.</div>
        {/if}
      {:else}
        <div class="diff-viewer-empty">
          <Icon icon={FileCode} size={28} class="empty-icon" />
          <span>Select a file to inspect diff</span>
        </div>
      {/if}
    </div>

    <DiffFileTree
      {files}
      {error}
      {emptyLabel}
      {selectedPath}
      {searchQuery}
      onSelect={(path) => void selectFile(path)}
      onSearchChange={(q) => (searchQuery = q)}
    />
  </div>
</aside>
