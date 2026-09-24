<script lang="ts">
  import { pushToast } from "$lib/runtime";
  import { copyFeedback } from "$lib/ui/copyFeedback.svelte";
  import Icon, { Check, Copy } from "$lib/ui/icons";

  type Props = { text: string };
  let { text }: Props = $props();
  const feedback = copyFeedback();

  async function copyMessage() {
    if (!(await feedback.copy(text))) pushToast("Copy failed", "warn");
  }
</script>

<div class="assistant-message-actions" role="group" aria-label="Assistant message actions">
  <button
    type="button"
    class={`bubble-action-icon-btn${feedback.copied ? " ok" : ""}`}
    title={feedback.copied ? "Copied" : "Copy response"}
    aria-label="Copy response"
    onclick={() => void copyMessage()}
  >
    <Icon icon={feedback.copied ? Check : Copy} size={11} />
    <span class="bubble-action-label">{feedback.copied ? "Copied" : "Copy"}</span>
  </button>
</div>

<style>
  .assistant-message-actions {
    display: flex;
    align-items: center;
    gap: 6px;
    min-height: 22px;
    margin-top: 3px;
    opacity: 0;
    pointer-events: none;
    transition: opacity 0.15s ease;
  }

  :global(.assistant-block:hover) .assistant-message-actions,
  :global(.assistant-block:focus-within) .assistant-message-actions {
    opacity: 1;
    pointer-events: auto;
  }
</style>
