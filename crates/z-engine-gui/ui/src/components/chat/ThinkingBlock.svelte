<script lang="ts">
  import { handleEvent } from "$lib/runtime";
  import type { Msg } from "$lib/types";
  import Icon, { ChevronDown, ChevronRight, Sparkles } from "$lib/ui/icons";

  type Props = { m: Msg };
  let { m }: Props = $props();

  const streaming = $derived(Boolean(m.streaming));
  const collapsed = $derived(Boolean(m.collapsed) && !streaming);
  const chars = $derived(m.thinkingBody?.length ?? 0);
</script>

{#if streaming}
  <div class="msg thinking streaming">
    <span class="thinking-head">
      <span class="reason-pulse-dot"></span>
      <Icon icon={Sparkles} size={12} class="thinking-icon" />
      <span>Thinking… ({chars} chars)</span>
    </span>
  </div>
{:else}
  <div class={`msg thinking${collapsed ? "" : " open"}`}>
    <button
      type="button"
      class="thinking-head"
      onclick={() => handleEvent({ type: "toggleThinking", id: m.id })}
      title={collapsed ? "Show thought process" : "Hide thought process"}
    >
      {#if collapsed}
        <Icon icon={ChevronRight} size={12} />
      {:else}
        <Icon icon={ChevronDown} size={12} />
      {/if}
      <Icon icon={Sparkles} size={12} class="thinking-icon" />
      <span>Thought process ({chars} chars)</span>
    </button>
    {#if !collapsed && m.thinkingBody}
      <pre class="thinking-body">{m.thinkingBody}</pre>
    {/if}
  </div>
{/if}
