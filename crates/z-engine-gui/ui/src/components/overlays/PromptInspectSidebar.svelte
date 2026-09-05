<script lang="ts">
  import {
    categorizeRow,
    categoryMeta,
    type ContextCategory,
    type InspectRow,
  } from "$lib/promptInspectView";
  import Icon, {
    Brain,
    FolderGit2,
    MessageSquare,
    Search,
    Sparkles,
    Terminal,
    User,
    Wrench,
    X,
  } from "$lib/ui/icons";
  import { fmtTokens } from "$lib/util";

  type FilterCategory = "all" | ContextCategory;

  type Props = {
    rows: InspectRow[];
    selected: number;
    onSelect: (index: number) => void;
    activeCategory: FilterCategory;
    onSelectCategory: (cat: FilterCategory) => void;
    loading?: boolean;
    err?: string | null;
  };

  let {
    rows,
    selected,
    onSelect,
    activeCategory,
    onSelectCategory,
    loading = false,
    err = null,
  }: Props = $props();

  let query = $state("");

  function getItemIcon(cat: ContextCategory, role: string) {
    if (cat === "instructions") return Brain;
    if (cat === "project") return FolderGit2;
    if (cat === "capabilities") return Wrench;
    if (role === "user") return User;
    if (role === "assistant") return Sparkles;
    return MessageSquare;
  }

  const items = $derived.by(() => {
    return rows.map((row, index) => {
      const cat = categorizeRow(row);
      const meta = categoryMeta(cat);
      const label = row.kind === "msg" ? row.part.label : row.tool.name;
      const tokens = row.kind === "msg" ? row.part.tokens : row.tool.tokens;
      const role = row.kind === "msg" ? row.part.role : "tool";

      const matchesCat = activeCategory === "all" || activeCategory === cat;
      const q = query.trim().toLowerCase();
      const matchesQuery =
        !q ||
        label.toLowerCase().includes(q) ||
        meta.label.toLowerCase().includes(q) ||
        role.toLowerCase().includes(q);

      return {
        row,
        index,
        cat,
        meta,
        label,
        role,
        tokens,
        visible: matchesCat && matchesQuery,
      };
    });
  });

  const visibleItems = $derived(items.filter((i) => i.visible));

  const counts = $derived.by(() => {
    const c = { all: rows.length, instructions: 0, project: 0, conversation: 0, capabilities: 0 };
    for (const r of rows) {
      const cat = categorizeRow(r);
      c[cat] = (c[cat] || 0) + 1;
    }
    return c;
  });
</script>

<aside class="sidebar settings-nav-island prompt-sidebar-island">
  <div class="sidebar-top-bar">
    <span class="prompt-sidebar-heading">Context Sections</span>
    <span class="prompt-parts-count">{visibleItems.length} of {rows.length}</span>
  </div>

  <div class="prompt-filter-bar">
    <div class="prompt-search-input-wrap">
      <Icon icon={Search} size={13} class="prompt-search-icon" />
      <input
        type="text"
        placeholder="Search context & rules…"
        bind:value={query}
        class="prompt-search-input"
        spellcheck={false}
      />
      {#if query}
        <button
          type="button"
          class="prompt-search-clear"
          onclick={() => (query = "")}
          aria-label="Clear search"
        >
          <Icon icon={X} size={11} />
        </button>
      {/if}
    </div>

    <div class="prompt-cat-tabs" role="tablist" aria-label="Filter category">
      <button
        type="button"
        class={`prompt-cat-tab${activeCategory === "all" ? " active" : ""}`}
        onclick={() => onSelectCategory("all")}
      >
        All
      </button>
      {#if counts.instructions > 0}
        <button
          type="button"
          class={`prompt-cat-tab${activeCategory === "instructions" ? " active" : ""}`}
          onclick={() => onSelectCategory("instructions")}
          title="Operating instructions and rules"
        >
          Rules
        </button>
      {/if}
      {#if counts.project > 0}
        <button
          type="button"
          class={`prompt-cat-tab${activeCategory === "project" ? " active" : ""}`}
          onclick={() => onSelectCategory("project")}
          title="Files, repository map, and workspace context"
        >
          Project
        </button>
      {/if}
      {#if counts.conversation > 0}
        <button
          type="button"
          class={`prompt-cat-tab${activeCategory === "conversation" ? " active" : ""}`}
          onclick={() => onSelectCategory("conversation")}
          title="Recent session conversation"
        >
          Chat
        </button>
      {/if}
      {#if counts.capabilities > 0}
        <button
          type="button"
          class={`prompt-cat-tab${activeCategory === "capabilities" ? " active" : ""}`}
          onclick={() => onSelectCategory("capabilities")}
          title="Authorized tools and capabilities"
        >
          Tools
        </button>
      {/if}
    </div>
  </div>

  <nav class="settings-nav prompt-rail-nav" aria-label="Context sections">
    {#if loading}
      <div class="prompt-rail-loading">Loading context…</div>
    {:else if err}
      <div class="prompt-rail-loading prompt-rail-err">{err}</div>
    {:else if visibleItems.length === 0}
      <div class="prompt-rail-loading">
        {query ? "No matching sections found" : "No context in this section"}
      </div>
    {:else}
      {#each visibleItems as item (item.index)}
        {@const isSelected = selected === item.index}
        <button
          type="button"
          class={`prompt-item-btn${isSelected ? " active" : ""}`}
          onclick={() => onSelect(item.index)}
        >
          <span class={`prompt-item-icon cat-${item.cat}`} style={`color: ${item.meta.color}`}>
            <Icon icon={getItemIcon(item.cat, item.role)} size={14} />
          </span>
          <div class="prompt-item-info">
            <span class="prompt-item-label" title={item.label}>{item.label}</span>
            <span class="prompt-item-sub">{item.meta.label}</span>
          </div>
          <span class="prompt-item-tok-tag">~{fmtTokens(item.tokens)}</span>
        </button>
      {/each}
    {/if}
  </nav>
</aside>

