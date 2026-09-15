<script lang="ts">
  import { pushToast } from "$lib/runtime";
  import { copyFeedback } from "$lib/ui";
  import Icon, { Check, Copy, Eye } from "$lib/ui/icons";

  type Props = {
    text: string;
    onInspect?: () => void;
    inspecting?: boolean;
  };

  let { text, onInspect, inspecting = false }: Props = $props();
  const feedback = copyFeedback();
  const copied = $derived(feedback.copied);

  async function copyMessage() {
    if (!(await feedback.copy(text))) pushToast("Copy failed", "warn");
  }
</script>

<div class="assistant-message-actions" role="group" aria-label="Assistant message actions">
  <button
    type="button"
    class={`bubble-action-icon-btn${copied ? " ok" : ""}`}
    title={copied ? "Copied" : "Copy response"}
    aria-label="Copy response"
    onclick={() => void copyMessage()}
  >
    <Icon icon={copied ? Check : Copy} size={11} />
    <span class="bubble-action-label">{copied ? "Copied" : "Copy"}</span>
  </button>

  {#if onInspect}
    <button
      type="button"
      class="bubble-action-icon-btn"
      title={inspecting ? "Hide process" : "Inspect process"}
      aria-label={inspecting ? "Hide process" : "Inspect process"}
      aria-pressed={inspecting}
      onclick={onInspect}
    >
      <Icon icon={Eye} size={11} />
      <span class="bubble-action-label">Inspect</span>
    </button>
  {/if}
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

  :global(.assistant-turn:hover) .assistant-message-actions,
  :global(.assistant-turn:focus-within) .assistant-message-actions {
    opacity: 1;
    pointer-events: auto;
  }
</style>
