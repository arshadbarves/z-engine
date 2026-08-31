<script lang="ts">
  import { catalogForPicker, catalogStore, fmtLimit } from "$lib/catalog";
  import { setModel } from "$lib/commands";
  import { modelStore } from "$lib/runtime";
  import { bindStore } from "$lib/svelte/bind.svelte";
  import Icon, { ChevronDown, Search, Sparkles } from "$lib/ui/icons";
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
          if (id === model.current) return false;
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
  <button class="mode model-btn" onclick={() => (open = !open)} title="Switch model">
    <Icon icon={Sparkles} size={11} />
    <span>{shortModel(model.current) || "model"}</span>
    <Icon icon={ChevronDown} size={9} strokeWidth={2.4} />
  </button>
  {#if open}
    <div class="popover popover-wide" role="menu">
      <div class="popover-head">Model</div>
      <div class="popover-current">{model.current || "(default from config)"}</div>
      <div class="pop-search">
        <Icon icon={Search} size={11} />
        <input
          bind:value={query}
          placeholder="Filter models…"
          spellcheck={false}
          autofocus
          onkeydown={(e) => e.key === "Escape" && (open = false)}
        />
      </div>
      <div class="popover-scroll">
        {#if groups.length === 0 && !query}
          <div class="pop-note">
            {catalog.current
              ? "No OpenRouter models — check Settings for your API key."
              : "loading catalog…"}
          </div>
        {/if}
        {#each groups as g}
          <div>
            <div class="palette-group">{g.provider}</div>
            {#each g.items as m}
              <button
                class="popover-item model-item"
                role="menuitem"
                onclick={() => void pick(m.id)}
              >
                <span class="model-name">{m.name}</span>
                <span class="popover-sub">
                  {m.reasoning ? "reasoning · " : ""}
                  {[fmtLimit(m.context), fmtLimit(m.output)].filter(Boolean).join(" / ") || m.id}
                </span>
              </button>
            {/each}
          </div>
        {/each}
      </div>
      <form
        class="popover-custom"
        onsubmit={(e) => {
          e.preventDefault();
          const id = custom.trim();
          if (id) void pick(id);
          custom = "";
        }}
      >
        <input bind:value={custom} placeholder="Custom model id…" spellcheck={false} />
        <button type="submit" disabled={!custom.trim()}>Set</button>
      </form>
    </div>
  {/if}
</div>
