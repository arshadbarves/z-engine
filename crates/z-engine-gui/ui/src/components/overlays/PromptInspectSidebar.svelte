<script lang="ts">
  import type { PromptLayer } from "$lib/promptInsights";
  import type { InspectRow } from "$lib/promptInspectView";
  import Icon, {
    Brain,
    Eye,
    Search,
    Sparkles,
    Terminal,
    User,
    Workflow,
    X,
    Zap,
  } from "$lib/ui/icons";
  import { fmtTokens } from "$lib/util";
  import LogoMark from "../chrome/LogoMark.svelte";

  type FilterKind = "all" | "message" | "tool";

  type Props = {
    rows: InspectRow[];
    layers: PromptLayer[];
    selected: number;
    onSelect: (index: number) => void;
    totalTokens: number;
    loading?: boolean;
    err?: string | null;
  };

  let {
    rows,
    layers,
    selected,
    onSelect,
    totalTokens,
    loading = false,
    err = null,
  }: Props = $props();

  let filter = $state<FilterKind>("all");
  let query = $state("");

  function getRoleIcon(role: string, kind: "msg" | "tool") {
    if (kind === "tool") return Workflow;
    if (role === "system") return Brain;
    if (role === "user") return User;
    if (role === "assistant") return Sparkles;
    if (role === "tool") return Terminal;
    return Eye;
  }

  const filteredItems = $derived.by(() => {
    return rows
      .map((row, index) => {
        const layer = layers[index];
        const label = row.kind === "msg" ? row.part.label : row.tool.name;
        const role = row.kind === "msg" ? row.part.role : "tool def";
        const tokens = row.kind === "msg" ? row.part.tokens : row.tool.tokens;
        const matchKind =
          filter === "all" ||
          (filter === "message" && row.kind === "msg") ||
          (filter === "tool" && row.kind === "tool");
        const matchQuery =
          !query.trim() ||
          label.toLowerCase().includes(query.toLowerCase().trim()) ||
          role.toLowerCase().includes(query.toLowerCase().trim());
        return {
          row,
          index,
          layer,
          label,
          role,
          tokens,
          visible: matchKind && matchQuery,
        };
      })
      .filter((item) => item.visible);
  });
</script>

<aside class="sidebar settings-nav-island prompt-sidebar-island">
  <div class="sidebar-top-bar">
    <div class="sidebar-brand-pill">
      <LogoMark size={18} />
      <span class="sidebar-brand-text">Prompt Structure</span>
    </div>
    <span class="prompt-parts-count">{rows.length} parts</span>
  </div>

  <div class="prompt-filter-bar">
    <div class="prompt-search-input-wrap">
      <Icon icon={Search} size={13} class="prompt-search-icon" />
      <input
        type="text"
        placeholder="Filter parts…"
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

    <div class="prompt-kind-tabs" role="tablist" aria-label="Filter kind">
      <button
        type="button"
        class={`prompt-kind-tab${filter === "all" ? " active" : ""}`}
        onclick={() => (filter = "all")}
      >
        All
      </button>
      <button
        type="button"
        class={`prompt-kind-tab${filter === "message" ? " active" : ""}`}
        onclick={() => (filter = "message")}
      >
        Messages
      </button>
      <button
        type="button"
        class={`prompt-kind-tab${filter === "tool" ? " active" : ""}`}
        onclick={() => (filter = "tool")}
      >
        Tools
      </button>
    </div>
  </div>

  <nav class="settings-nav prompt-rail-nav" aria-label="Prompt parts">
    {#if loading}
      <div class="prompt-rail-loading">Analyzing prompt assembly…</div>
    {:else if err}
      <div class="prompt-rail-loading prompt-rail-err">{err}</div>
    {:else if filteredItems.length === 0}
      <div class="prompt-rail-loading">
        {query ? "No matching parts found" : "No prompt parts"}
      </div>
    {:else}
      {#each filteredItems as item}
        {@const isSelected = selected === item.index}
        {@const share = totalTokens > 0 ? (item.tokens / totalTokens) * 100 : 0}
        <button
          type="button"
          class={`prompt-item-btn${isSelected ? " active" : ""}`}
          onclick={() => onSelect(item.index)}
        >
          <div class="prompt-item-left">
            <span class={`prompt-role-icon role-${item.role.replace(/\s+/g, "-")}`}>
              <Icon icon={getRoleIcon(item.role, item.row.kind)} size={13} />
            </span>
            <div class="prompt-item-info">
              <div class="prompt-item-title-row">
                <span class="prompt-item-ord">#{item.layer?.order ?? item.index + 1}</span>
                <span class="prompt-item-label" title={item.label}>{item.label}</span>
                {#if item.layer?.cacheable}
                  <span class="prompt-cache-tag cacheable" title={`Prefix Cacheable (~${fmtTokens(item.tokens)} tok)`}>
                    <Icon icon={Zap} size={9} />
                  </span>
                {/if}
              </div>
              <div class="prompt-item-sub">
                <span class="prompt-item-role">{item.role}</span>
                <span class="prompt-item-dot">·</span>
                <span class="prompt-item-tok">~{fmtTokens(item.tokens)} ({Math.round(share)}%)</span>
              </div>
            </div>
          </div>

          <div class="prompt-item-bar-track">
            <div
              class={`prompt-item-bar-fill role-${item.role.replace(/\s+/g, "-")}`}
              style={`width: ${Math.max(4, Math.min(100, share))}%;`}
            ></div>
          </div>
        </button>
      {/each}
    {/if}
  </nav>

  <div class="settings-rail-foot prompt-rail-footer">
    <div class="prompt-footer-row">
      <span>Total Context</span>
      <strong>~{fmtTokens(totalTokens)} tokens</strong>
    </div>
  </div>
</aside>
