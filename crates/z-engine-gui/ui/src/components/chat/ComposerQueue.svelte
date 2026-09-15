<script lang="ts">
  import { queuePreview, queueTitle } from "$lib/domain/queuePreview";
  import type { QueuedMessage } from "$lib/types";
  import Icon, { X } from "$lib/ui/icons";

  type Props = { items: QueuedMessage[]; onRemove: (index: number) => void };
  let { items, onRemove }: Props = $props();
</script>

{#if items.length > 0}
  <div class="queue-strip" role="group" aria-label="Queued follow-ups">
    <span class="queue-label">queued</span>
    {#each items as item, i}
      <span class="queue-pill" title={queueTitle(item)}>
        <span class="queue-pill-text">{queuePreview(item)}</span>
        <button
          type="button"
          class="queue-pill-x"
          aria-label={`Remove queued follow-up ${i + 1}`}
          onclick={() => onRemove(i)}
        >
          <Icon icon={X} size={9} strokeWidth={2.4} />
        </button>
      </span>
    {/each}
  </div>
{/if}
