<script lang="ts">
  import { contextBreakdown } from "$lib/commands";
  import type { CtxMeter } from "$lib/domain/contextMeter";
  import { compact, pushToast } from "$lib/runtime";
  import { settingsStore } from "$lib/stores/settings.svelte";
  import { ui } from "$lib/stores/ui.svelte";
  import { fmtTokens } from "$lib/util";

  /** What fills the context window, layer by layer, with the two things to do about it. */
  type Props = { sessionId: string; meter: CtxMeter; busy: boolean; onClose: () => void };
  let { sessionId, meter, busy, onClose }: Props = $props();

  let loading = $state(true);
  let compacting = $state(false);
  const compactAt = $derived(settingsStore.settings?.context.compact_at_percent ?? null);

  $effect(() => {
    const id = sessionId;
    loading = true;
    void contextBreakdown(id)
      .catch(() => null)
      .finally(() => (loading = false));
  });

  function share(tokens: number): string {
    return `${Math.max(0, Math.round((tokens / meter.max) * 100))}%`;
  }

  async function onCompact() {
    compacting = true;
    if (await compact()) {
      pushToast("Compacting the conversation…", "info");
      onClose();
    }
    compacting = false;
  }

  function onInspect() {
    onClose();
    ui.inspectOpen = true;
  }
</script>

<div class="context-card-body">
  <header class="context-head">
    <p class="context-title">Context</p>
    <p class={`context-pct level-${meter.level}`}>{meter.pct}% used</p>
    <p class="context-sub">
      {fmtTokens(meter.used)} of {fmtTokens(meter.max)} tokens · {fmtTokens(meter.remaining)} left
    </p>
  </header>

  <div class="context-bar" class:is-loading={loading && meter.totalOnly} aria-hidden="true">
    {#each meter.slices as slice (slice.id)}
      <span style:width={share(slice.tokens)} style:background={slice.color}></span>
    {/each}
  </div>

  {#if !meter.totalOnly}
    <ul class="context-legend">
      {#each meter.slices as slice (slice.id)}
        <li>
          <span class="context-swatch" style:background={slice.color}></span>
          <span class="context-layer">{slice.label}</span>
          <span class="context-tokens">{fmtTokens(slice.tokens)}</span>
        </li>
      {/each}
    </ul>
  {:else if !loading}
    <p class="context-note">The breakdown appears after the first reply.</p>
  {/if}

  {#if compactAt}
    <p class="context-note">Z Engine compacts older messages by itself at {compactAt}%.</p>
  {/if}

  <div class="context-actions">
    <button type="button" class="btn-secondary" disabled={compacting || busy} onclick={onCompact}>Compact now</button>
    <button type="button" class="btn-ghost" onclick={onInspect}>Inspect prompt</button>
  </div>
</div>
