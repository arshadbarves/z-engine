<script lang="ts">
  import { usageStore } from "$lib/runtime/state";
  import { bindStore } from "$lib/svelte/bind.svelte";

  type Props = {
    busy?: boolean;
    isApproval?: boolean;
    speedMode?: "fast" | "deep";
    onSpeedChange?: (mode: "fast" | "deep") => void;
  };

  let {
    busy = false,
    isApproval = false,
    speedMode = "fast",
    onSpeedChange,
  }: Props = $props();

  const usage = bindStore(usageStore);

  const totalTokens = $derived(
    (usage.current?.promptTokens ?? 0) + (usage.current?.completionTokens ?? 0),
  );

  const formattedTokens = $derived.by(() => {
    if (totalTokens > 1000) return `${(totalTokens / 1000).toFixed(1)}k tokens`;
    return `${totalTokens} tokens`;
  });
</script>

<div class="stage-toolbar" role="toolbar" aria-label="Stage status toolbar">
  <div style="display:flex; align-items:center; gap:10px;">
    <div
      class={`agent-status-pill${busy ? " synthesizing" : ""}`}
      aria-live="polite"
    >
      <div
        class={`pulse-dot${busy ? " purple" : isApproval ? "" : " green"}`}
        aria-hidden="true"
      ></div>
      <span>
        {#if busy}
          Z Engine Synthesizing…
        {:else if isApproval}
          Needs Approval
        {:else}
          Z Engine Standby
        {/if}
      </span>
    </div>

    <span style="font-size:11px; color:var(--text-dim);">
      Context: {formattedTokens}
    </span>
  </div>

  <div class="segmented" role="group" aria-label="Reasoning speed mode">
    <button
      type="button"
      class={`seg-btn${speedMode === "fast" ? " active" : ""}`}
      onclick={() => onSpeedChange?.("fast")}
    >
      Fast (Sonnet)
    </button>
    <button
      type="button"
      class={`seg-btn${speedMode === "deep" ? " active" : ""}`}
      onclick={() => onSpeedChange?.("deep")}
    >
      Deep Reasoning (o3)
    </button>
  </div>
</div>
