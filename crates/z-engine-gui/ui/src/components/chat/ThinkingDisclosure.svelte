<script lang="ts">
  import Icon, { ChevronDown, ChevronRight, Sparkles } from "$lib/ui/icons";

  type Props = { text: string; streaming?: boolean; redacted?: boolean };
  let { text, streaming = false, redacted = false }: Props = $props();

  let open = $state(false);
  const chars = $derived(text.length >= 1000 ? `${(text.length / 1000).toFixed(1)}k chars` : `${text.length} chars`);
  const label = $derived(redacted ? "Reasoning (redacted)" : streaming ? "Reasoning…" : "Thought process");
</script>

<div class={`msg thinking${open ? " open" : ""}${streaming ? " streaming" : ""}`}>
  <button
    type="button"
    class="thinking-head"
    disabled={redacted || !text}
    aria-expanded={redacted ? undefined : open}
    onclick={() => (open = !open)}
  >
    {#if streaming}
      <span class="reason-pulse-dot" aria-hidden="true"></span>
    {:else}
      <Icon icon={open ? ChevronDown : ChevronRight} size={11} />
    {/if}
    <Icon icon={Sparkles} size={11} class="thinking-icon" />
    <span class="thinking-label">{label}</span>
    {#if text}<span class="thinking-metric">{chars}</span>{/if}
  </button>
  {#if open && text}
    <pre class="thinking-body">{text}</pre>
  {/if}
</div>
