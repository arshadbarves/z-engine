<script lang="ts">
  import { onMount } from "svelte";
  import { diffForFile, listChangedFiles, type ChangedFile } from "$lib/commands";
  import { looksLikeDiff } from "$lib/diffParse";
  import { ChevronDown, ChevronRight, FileCode, Icon, RefreshCw, X } from "$lib/ui/icons";
  import DiffView from "./DiffView.svelte";

  type Props = {
    isClosing?: boolean;
    onClose: () => void;
  };

  let { isClosing = false, onClose }: Props = $props();

  let files = $state<ChangedFile[] | null>(null);
  let error = $state<string | null>(null);
  let openPath = $state<string | null>(null);
  let diff = $state("");
  let refreshing = $state(false);

  onMount(() => {
    let active = true;
    void listChangedFiles()
      .then((f) => {
        if (active) {
          files = f;
          error = null;
        }
      })
      .catch((e) => {
        if (active) error = String(e);
      });
    return () => {
      active = false;
    };
  });

  async function refresh() {
    refreshing = true;
    try {
      files = await listChangedFiles();
      error = null;
    } catch (e) {
      error = String(e);
    } finally {
      refreshing = false;
    }
  }

  async function toggle(path: string) {
    if (openPath === path) {
      openPath = null;
      return;
    }
    openPath = path;
    diff = "";
    try {
      diff = await diffForFile(path);
    } catch (e) {
      diff = `(no diff available: ${String(e)})`;
    }
  }
</script>

<aside class="diff-panel{isClosing ? ' is-closing' : ''}">
  <div class="diff-head">
    <div class="diff-head-left">
      <span class="diff-title">Workbench Changes</span>
      {#if files && files.length > 0}
        <span class="diff-count-badge">{files.length}</span>
      {/if}
    </div>
    <div class="diff-head-actions">
      <button
        type="button"
        class="icon-btn{refreshing ? ' spinning' : ''}"
        title="Refresh changes"
        onclick={() => void refresh()}
      >
        <Icon icon={RefreshCw} size={12} />
      </button>
      <button type="button" class="icon-btn" title="Close review pane" onclick={onClose}>
        <Icon icon={X} size={13} />
      </button>
    </div>
  </div>
  <div class="diff-body">
    {#if files === null && !error}
      <div class="sess-empty">Checking git status…</div>
    {/if}
    {#if error}
      <div class="sess-empty">Git status unavailable: {error}</div>
    {/if}
    {#if files?.length === 0}
      <div class="sess-empty">Working tree is clean — no modified files.</div>
    {/if}
    {#each files ?? [] as f (f.path)}
      {@const isOpen = openPath === f.path}
      <div class="diff-file{isOpen ? ' is-open' : ''}">
        <button
          type="button"
          class="diff-file-head status-{f.status}"
          onclick={() => void toggle(f.path)}
        >
          {#if isOpen}
            <Icon icon={ChevronDown} size={12} />
          {:else}
            <Icon icon={ChevronRight} size={12} />
          {/if}
          <Icon icon={FileCode} size={13} class="diff-file-icon" />
          <span class="diff-path">{f.path}</span>
          <span class="badge {f.status}">{f.status}</span>
        </button>
        {#if isOpen}
          {#if diff === ""}
            <pre class="diff-text">Loading diff…</pre>
          {:else if looksLikeDiff(diff)}
            <DiffView text={diff} />
          {:else}
            <pre class="diff-text">{diff}</pre>
          {/if}
        {/if}
      </div>
    {/each}
  </div>
</aside>
