<script lang="ts">
  import { onMount } from "svelte";
  import {
    diffForFile,
    listChangedFiles,
    listSessionChangedFiles,
    sessionDiffForFile,
    type ChangedFile,
  } from "$lib/commands";
  import { sessionStore } from "$lib/runtime/state";
  import { bindStore } from "$lib/svelte/bind.svelte";
  import Icon, { FileCode, RefreshCw } from "$lib/ui/icons";
  import DiffFileTree from "../overlays/DiffFileTree.svelte";
  import DiffView from "../overlays/DiffView.svelte";

  type Scope = "session" | "git";

  const sessionId = bindStore(sessionStore);
  let scope = $state<Scope>("session");
  let files = $state<ChangedFile[] | null>(null);
  let error = $state<string | null>(null);
  let selectedPath = $state<string | null>(null);
  let diffCache = $state<Record<string, string>>({});
  let loadingDiff = $state(false);
  let refreshing = $state(false);
  let searchQuery = $state("");
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

  const activeDiff = $derived(
    selectedPath ? diffCache[`${scope}:${selectedPath}`] ?? null : null,
  );

  const activeFileStats = $derived(
    files?.find((f) => f.path === selectedPath) ?? null,
  );
</script>

<div class="deck-content active" id="deck-diff">
  <div style="display:flex; justify-content:space-between; align-items:center; padding:8px 12px; background:rgba(0,0,0,0.25); border-bottom:1px solid var(--border-dark);">
    <div class="segmented">
      <button
        type="button"
        class={`seg-btn${scope === "session" ? " active" : ""}`}
        onclick={() => void setScope("session")}
      >
        Chat
      </button>
      <button
        type="button"
        class={`seg-btn${scope === "git" ? " active" : ""}`}
        onclick={() => void setScope("git")}
      >
        Git
      </button>
    </div>

    <div style="display:flex; align-items:center; gap:6px;">
      {#if activeFileStats}
        <span style="font-family:var(--mono); font-size:10px; color:var(--apple-green);">
          +{activeFileStats.added} / -{activeFileStats.deleted} lines
        </span>
      {/if}
      <button
        type="button"
        class="icon-btn"
        style="width:24px; height:24px;"
        title="Refresh diffs"
        onclick={() => void refresh()}
      >
        <Icon icon={RefreshCw} size={12} class={refreshing ? "spinning" : ""} />
      </button>
    </div>
  </div>

  <div style="flex:1; overflow-y:auto; display:flex; flex-direction:column;">
    {#if selectedPath && activeDiff}
      <div style="padding:6px 12px; font-family:var(--mono); font-size:11px; color:var(--text-secondary); background:rgba(255,255,255,0.02); border-bottom:1px solid var(--border-subtle);">
        {selectedPath}
      </div>
      <div style="flex:1; overflow-y:auto;">
        <DiffView text={activeDiff} filePath={selectedPath} />
      </div>
    {:else if loadingDiff}
      <div style="padding:20px; text-align:center; color:var(--text-dim); font-size:11px;">
        Loading diff…
      </div>
    {:else}
      <div style="padding:24px; text-align:center; color:var(--text-dim); font-size:11px; display:flex; flex-direction:column; align-items:center; gap:8px;">
        <Icon icon={FileCode} size={24} />
        <span>Select a file to inspect diff</span>
      </div>
    {/if}

    {#if files && files.length > 0}
      <div style="border-top:1px solid var(--border-dark); max-height:160px; overflow-y:auto;">
        <DiffFileTree
          {files}
          {error}
          emptyLabel="No modified files"
          {selectedPath}
          {searchQuery}
          onSelect={(path) => void selectFile(path)}
          onSearchChange={(q) => (searchQuery = q)}
        />
      </div>
    {/if}
  </div>
</div>
