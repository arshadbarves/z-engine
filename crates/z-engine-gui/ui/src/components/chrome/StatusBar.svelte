<script lang="ts">
  import { usageStore } from "$lib/runtime/state";
  import { bindStore } from "$lib/svelte/bind.svelte";

  type Props = {
    model?: string;
    branch?: string | null;
    workspaceName?: string | null;
    mode?: "autonomous" | "pair";
    busy?: boolean;
  };

  let {
    model = "Codex 4.5 Sonnet",
    branch = null,
    workspaceName = null,
    mode = "autonomous",
    busy = false,
  }: Props = $props();

  const usage = bindStore(usageStore);

  const totalTokens = $derived(
    (usage.current?.promptTokens ?? 0) + (usage.current?.completionTokens ?? 0),
  );

  const maxTokens = $derived(usage.current?.maxTokens ?? 128_000);

  const formattedTokens = $derived.by(() => {
    const kCurrent = totalTokens > 1000 ? `${(totalTokens / 1000).toFixed(1)}k` : `${totalTokens}`;
    const kMax = maxTokens > 1000 ? `${Math.round(maxTokens / 1000)}k` : `${maxTokens}`;
    return `${kCurrent} / ${kMax}`;
  });

  const estimatedCost = $derived.by(() => {
    const prompt = usage.current?.promptTokens ?? 0;
    const comp = usage.current?.completionTokens ?? 0;
    const cost = (prompt * 3 + comp * 15) / 1_000_000;
    return `$${cost.toFixed(3)}`;
  });
</script>

<footer id="statusbar" aria-label="Status Bar">
  <div class="status-item">
    <span class={`pulse-dot green${busy ? " purple" : ""}`} aria-hidden="true"></span>
    <span>Engine: <span class="status-val">{model}</span></span>
  </div>

  <div class="status-sep" aria-hidden="true"></div>

  <div class="status-item">
    <span>Tokens:</span>
    <span class="status-val">{formattedTokens}</span>
  </div>

  <div class="status-sep" aria-hidden="true"></div>

  <div class="status-item">
    <span>Cost:</span>
    <span class="status-val">{estimatedCost}</span>
  </div>

  <div class="status-sep" aria-hidden="true"></div>

  <div class="status-item">
    <span>Speed:</span>
    <span class="status-val">{busy ? "74 tok/s" : "idle"}</span>
  </div>

  <div style="margin-left:auto; display:flex; gap:16px; align-items:center;">
    <div class="status-item">
      <span>Autonomy:</span>
      <span
        class="status-val"
        style={mode === "autonomous"
          ? "color:var(--apple-green);"
          : "color:var(--apple-orange);"}
      >
        {mode === "autonomous" ? "Autonomous" : "Supervised"}
      </span>
    </div>

    {#if branch || workspaceName}
      <div class="status-sep" aria-hidden="true"></div>
      <div class="status-item">
        <span>Branch:</span>
        <span class="status-val">{branch || workspaceName}</span>
      </div>
    {/if}
  </div>
</footer>
