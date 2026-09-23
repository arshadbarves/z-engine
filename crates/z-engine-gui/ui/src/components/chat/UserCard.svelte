<script lang="ts">
  import { mediaSrc, messageDocuments, messageImages, visibleText } from "$lib/domain/timeline/blocks";
  import type { Message } from "$lib/protocol/Message";
  import type { RewindScope } from "$lib/protocol/RewindScope";
  import { pushToast } from "$lib/runtime";
  import { copyFeedback } from "$lib/ui/copyFeedback.svelte";
  import Icon, { Check, Copy, FileText } from "$lib/ui/icons";
  import RewindMenu from "./RewindMenu.svelte";

  const COLLAPSE_CHARS = 380;
  const COLLAPSE_LINES = 6;

  type Props = {
    message: Message;
    canRestoreCode?: boolean;
    onRewind?: (message: Message, scope: RewindScope) => void;
  };
  let { message, canRestoreCode = false, onRewind }: Props = $props();

  const feedback = copyFeedback();
  const text = $derived(visibleText(message));
  const images = $derived(messageImages(message));
  const documents = $derived(messageDocuments(message));
  const isLong = $derived(text.length > COLLAPSE_CHARS || text.split("\n").length > COLLAPSE_LINES);
  let expanded = $state(false);

  async function copy() {
    if (!(await feedback.copy(text))) pushToast("Copy failed", "warn");
  }
</script>

<div class="user-message-row" id={`msg-${message.id}`} data-msg-id={message.id}>
  <div class="user-message-wrapper">
    <div class="user-message-bubble">
      {#if text}
        <div class={`user-prompt-text${isLong && !expanded ? " collapsed" : ""}`}>{text}</div>
      {/if}
      {#if images.length > 0}
        <div class="user-attached-images">
          {#each images as source, i (i)}
            <img src={mediaSrc(source)} alt={`attachment ${i + 1}`} class="user-img-thumb" />
          {/each}
        </div>
      {/if}
      {#if documents.length > 0}
        <div class="user-attached-docs">
          {#each documents as doc, i (i)}
            <span class="user-doc-chip"><Icon icon={FileText} size={11} />{doc.title ?? "Document"}</span>
          {/each}
        </div>
      {/if}
      {#if isLong}
        <button type="button" class="user-expand-btn" onclick={() => (expanded = !expanded)}>
          {expanded ? "Show less" : "Show more"}
        </button>
      {/if}
    </div>

    <div class="user-bubble-actions">
      <button
        type="button"
        class={`bubble-action-icon-btn${feedback.copied ? " ok" : ""}`}
        title={feedback.copied ? "Copied" : "Copy prompt"}
        aria-label="Copy prompt"
        onclick={() => void copy()}
      >
        <Icon icon={feedback.copied ? Check : Copy} size={11} strokeWidth={1.8} />
        <span class="bubble-action-label">{feedback.copied ? "Copied" : "Copy"}</span>
      </button>
      {#if onRewind}
        <RewindMenu {canRestoreCode} onRewind={(scope) => onRewind(message, scope)} />
      {/if}
    </div>
  </div>
</div>
