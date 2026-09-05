<script lang="ts">
  import { catalogForPicker, catalogStore, fmtLimit } from "$lib/catalog";
  import { setModel } from "$lib/commands";
  import { modelStore } from "$lib/runtime";
  import { bindStore } from "$lib/svelte/bind.svelte";
  import Icon, { Brain, Check, ChevronDown, Search, Sparkles, X } from "$lib/ui/icons";
  import { shortModel } from "$lib/util";

  const model = bindStore(modelStore);
  const catalog = bindStore(catalogStore);
  let open = $state(false);
  let custom = $state("");
  let query = $state("");

  $effect(() => {
    if (open) void catalogStore.ensure();
  });

  async function pick(id: string) {
    open = false;
    query = "";
    if (id === model.current) return;
    try {
      await setModel(id);
      modelStore.set(id);
    } catch (e) {
      console.error(e);
    }
  }

  const groups = $derived.by(() => {
    const q = query.trim().toLowerCase();
    const out: {
      provider: string;
      items: { id: string; name: string; context?: number; output?: number; reasoning: boolean }[];
    }[] = [];
    if (!catalog.current) return out;
    const filtered = catalogForPicker(catalog.current);
    for (const [pid, prov] of Object.entries(filtered)) {
      const items = Object.entries(prov.models)
        .filter(([id, m]) => {
          if (!q) return true;
          return (
            id.toLowerCase().includes(q) ||
            m.name.toLowerCase().includes(q) ||
            prov.name.toLowerCase().includes(q)
          );
        })
        .slice(0, 40)
        .map(([id, m]) => ({
          id,
          name: m.name,
          context: m.context,
          output: m.output,
          reasoning: m.reasoning,
        }));
      if (items.length > 0) out.push({ provider: prov.name || pid, items });
    }
    out.sort((a, b) => a.provider.localeCompare(b.provider));
    return out;
  });
</script>

<div class="model-picker">
  {#if open}
    <div class="popover-backdrop" onclick={() => (open = false)}></div>
  {/if}
  <button
    class={`mode model-btn${open ? " is-open" : ""}`}
    onclick={() => (open = !open)}
    title="Switch model"
  >
    <Icon icon={Sparkles} size={12} class="model-sparkle-icon" />
    <span>{shortModel(model.current) || "model"}</span>
    <Icon icon={ChevronDown} size={10} strokeWidth={2} class="model-chevron-icon" />
  </button>
  {#if open}
    <div class="popover popover-wide model-picker-window" role="menu">
      <div class="model-picker-header">
        <div class="model-picker-title-row">
          <span class="model-picker-title">Model</span>
          {#if model.current}
            <div class="model-active-badge" title={`Active model: ${model.current}`}>
              <span class="model-active-dot" aria-hidden="true"></span>
              <span class="model-active-label">{shortModel(model.current)}</span>
            </div>
          {/if}
        </div>
        <div class="model-search-box">
          <Icon icon={Search} size={12} class="model-search-icon" />
          <input
            bind:value={query}
            placeholder="Search models or providers…"
            spellcheck={false}
            autofocus
            onkeydown={(e) => e.key === "Escape" && (open = false)}
          />
          {#if query}
            <button
              type="button"
              class="model-search-clear"
              onclick={() => (query = "")}
              aria-label="Clear filter"
            >
              <Icon icon={X} size={11} />
            </button>
          {/if}
        </div>
      </div>
      <div class="popover-scroll model-picker-scroll">
        {#if groups.length === 0 && !query}
          <div class="model-empty-note">
            {catalog.current
              ? "No OpenRouter models — check Settings for your API key."
              : "Loading catalog…"}
          </div>
        {:else if groups.length === 0 && query}
          <div class="model-empty-note">
            No models matching "{query}"
          </div>
        {/if}
        {#each groups as g}
          <div class="model-provider-group">
            <div class="model-provider-name">{g.provider}</div>
            {#each g.items as m}
              <button
                class={`model-picker-row${m.id === model.current ? " active" : ""}`}
                role="menuitem"
                onclick={() => void pick(m.id)}
              >
                <div class="model-row-left">
                  <div class="model-row-name-line">
                    <span class="model-row-name">{m.name}</span>
                    {#if m.reasoning}
                      <span class="model-chip-reasoning">
                        <Icon icon={Brain} size={9} />
                        <span>Reasoning</span>
                      </span>
                    {/if}
                  </div>
                  <div class="model-row-sub">
                    <span class="model-row-id">{m.id}</span>
                  </div>
                </div>
                <div class="model-row-right">
                  {#if m.context || m.output}
                    <span class="model-chip-spec">
                      {[fmtLimit(m.context), fmtLimit(m.output)].filter(Boolean).join(" / ")}
                    </span>
                  {/if}
                  {#if m.id === model.current}
                    <span class="model-active-check">
                      <Icon icon={Check} size={12} strokeWidth={2.4} />
                    </span>
                  {/if}
                </div>
              </button>
            {/each}
          </div>
        {/each}
      </div>
      <form
        class="model-picker-footer"
        onsubmit={(e) => {
          e.preventDefault();
          const id = custom.trim();
          if (id) void pick(id);
          custom = "";
        }}
      >
        <div class="model-custom-input-wrap">
          <input
            bind:value={custom}
            placeholder="Custom model ID (e.g. anthropic/claude-3.7-sonnet)…"
            spellcheck={false}
          />
        </div>
        <button
          type="submit"
          class="model-custom-btn"
          disabled={!custom.trim()}
        >
          Set
        </button>
      </form>
    </div>
  {/if}
</div>
