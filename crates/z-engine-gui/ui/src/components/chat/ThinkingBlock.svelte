<script lang="ts">
  import { handleEvent } from "$lib/runtime";
  import type { Msg } from "$lib/types";
  import Icon, { ChevronDown, ChevronRight, Sparkles } from "$lib/ui/icons";

  type Props = { m: Msg };
  let { m }: Props = $props();

  const streaming = $derived(Boolean(m.streaming));
  const collapsed = $derived(Boolean(m.collapsed) && !streaming);
  const chars = $derived(m.thinkingBody?.length ?? 0);
  const charDisplay = $derived(
    chars >= 1000 ? `${(chars / 1000).toFixed(1)}k chars` : `${chars} chars`,
  );
</script>

{#if streaming}
  <div class="msg thinking streaming">
    <span class="thinking-head">
      <span class="reason-pulse-dot"></span>
      <Icon icon={Sparkles} size={11} class="thinking-icon" />
      <span class="thinking-label">Reasoning…</span>
      <span class="thinking-metric">{charDisplay}</span>
    </span>
  </div>
{:else}
  <div class={`msg thinking${collapsed ? "" : " open"}`}>
    <button
      type="button"
      class="thinking-head"
      onclick={() => handleEvent({ type: "toggleThinking", id: m.id })}
      title={collapsed ? "Show thought process" : "Hide thought process"}
      aria-expanded={!collapsed}
    >
      <Icon icon={collapsed ? ChevronRight : ChevronDown} size={11} />
      <Icon icon={Sparkles} size={11} class="thinking-icon" />
      <span class="thinking-label">Thought process</span>
      <span class="thinking-metric">{charDisplay}</span>
    </button>
    {#if !collapsed && m.thinkingBody}
      <pre class="thinking-body">{m.thinkingBody}</pre>
    {/if}
  </div>
{/if}
