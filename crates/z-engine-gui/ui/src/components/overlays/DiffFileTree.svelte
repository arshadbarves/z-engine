<script lang="ts">
  import {
    buildDiffTree,
    expandAncestors,
    filterDiffTree,
    type DiffTreeFile,
    type DiffTreeNode,
  } from "$lib/domain/diffTree";
  import Icon, { ChevronDown, ChevronRight, FileCode, Folder, Search, X } from "$lib/ui/icons";

  type Props = {
    files: DiffTreeFile[] | null;
    error: string | null;
    emptyLabel: string;
    selectedPath: string | null;
    searchQuery?: string;
    onSelect: (path: string) => void;
    onSearchChange?: (query: string) => void;
  };

  let {
    files,
    error,
    emptyLabel,
    selectedPath,
    searchQuery = "",
    onSelect,
    onSearchChange,
  }: Props = $props();

  let collapsed = $state<Record<string, boolean>>({});
  let seededFor = $state<string | null>(null);

  function setSearch(q: string) {
    onSearchChange?.(q);
  }

  const tree = $derived.by(() => {
    if (!files) return [] as DiffTreeNode[];
    return filterDiffTree(buildDiffTree(files), searchQuery);
  });

  $effect(() => {
    if (!selectedPath || seededFor === selectedPath) return;
    seededFor = selectedPath;
    const next = { ...collapsed };
    for (const p of expandAncestors(selectedPath)) {
      delete next[p];
    }
    collapsed = next;
  });

  function toggleDir(path: string) {
    collapsed = { ...collapsed, [path]: !collapsed[path] };
  }

  function isOpen(path: string): boolean {
    if (searchQuery.trim()) return true;
    return !collapsed[path];
  }

  function statusLetter(status: string): string {
    if (status === "added") return "A";
    if (status === "deleted") return "D";
    return "M";
  }
</script>

<div class="diff-files-sidebar">
  <div class="diff-filter-bar">
    <div class="diff-search-wrap">
      <Icon icon={Search} size={12} class="diff-search-icon" />
      <input
        type="text"
        value={searchQuery}
        oninput={(e) => setSearch(e.currentTarget.value)}
        placeholder="Filter files…"
        spellcheck={false}
        class="diff-search-input"
      />
      {#if searchQuery}
        <button type="button" class="diff-clear-search" onclick={() => setSearch("")}>
          <Icon icon={X} size={10} />
        </button>
      {/if}
    </div>
  </div>

  <div class="diff-file-list-scroll diff-tree-scroll">
    {#if files === null && !error}
      <div class="diff-empty-hint">Scanning changes…</div>
    {:else if error}
      <div class="diff-empty-hint error">{error}</div>
    {:else if files?.length === 0}
      <div class="diff-empty-hint clean">{emptyLabel}</div>
    {:else if tree.length === 0}
      <div class="diff-empty-hint">No matches</div>
    {:else}
      {#snippet renderNodes(nodes: DiffTreeNode[], depth: number)}
        {#each nodes as node (node.path + node.kind)}
          {#if node.kind === "dir"}
            {@const open = isOpen(node.path)}
            <button
              type="button"
              class="diff-tree-row dir"
              style={`--depth: ${depth}`}
              onclick={() => toggleDir(node.path)}
            >
              <Icon icon={open ? ChevronDown : ChevronRight} size={11} class="diff-tree-chevron" />
              <Icon icon={Folder} size={13} class="diff-tree-folder" />
              <span class="diff-tree-name">{node.name}</span>
            </button>
            {#if open}
              {@render renderNodes(node.children, depth + 1)}
            {/if}
          {:else}
            {@const selected = selectedPath === node.path}
            <button
              type="button"
              class={`diff-tree-row file${selected ? " selected" : ""}`}
              style={`--depth: ${depth}`}
              title={`${node.status} · +${node.added} −${node.deleted}`}
              onclick={() => onSelect(node.path)}
            >
              <span class={`diff-status-dot ${node.status}`} aria-hidden="true"></span>
              <Icon icon={FileCode} size={13} class={`diff-tree-file-icon ${node.status}`} />
              <span class="diff-tree-name">{node.name}</span>
              {#if node.added > 0 || node.deleted > 0}
                <span class="diff-tree-stat">
                  {#if node.added > 0}<span class="add">+{node.added}</span>{/if}
                  {#if node.deleted > 0}<span class="del">−{node.deleted}</span>{/if}
                </span>
              {/if}
              <span class={`diff-status-letter ${node.status}`}>{statusLetter(node.status)}</span>
            </button>
          {/if}
        {/each}
      {/snippet}
      {@render renderNodes(tree, 0)}
    {/if}
  </div>
</div>
