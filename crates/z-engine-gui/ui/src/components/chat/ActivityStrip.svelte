<script lang="ts">
  import { splitWork } from "$lib/toolGroups";
  import type { Msg } from "$lib/types";
  import Icon, { ChevronRight } from "$lib/ui/icons";
  import ActionCard from "./ActionCard.svelte";

  type Props = { items: Msg[] };
  let { items }: Props = $props();

  const parts = $derived(splitWork(items));
  let openIds = $state<Record<number, boolean>>({});

  function toggle(id: number) {
    openIds = { ...openIds, [id]: !openIds[id] };
  }
</script>

<div class="activity-strip">
  {#each parts as p}
    {#if p.type === "reason"}
      {@const body = p.msg.thinkingBody}
      {@const isStreaming = Boolean(p.msg.streaming)}
      {@const open = Boolean(openIds[p.msg.id])}
      <div class={`reason-line${isStreaming ? " streaming" : ""}`}>
        <button
          type="button"
          class="reason-btn"
          onclick={() => body && toggle(p.msg.id)}
          disabled={!body && !isStreaming}
        >
          <span class={`reason-chevron${open ? " open" : ""}`}>
            <Icon icon={ChevronRight} size={11} />
          </span>
          <span class="reason-label">
            {#if isStreaming}<span class="reason-pulse-dot" aria-hidden="true"></span>{/if}
            Reasoning
          </span>
          <span class="reason-text">{p.text || (isStreaming ? "thinking…" : "")}</span>
        </button>
        {#if open && body}
          <pre class="thinking-body">{body}</pre>
        {/if}
      </div>
    {:else}
      <ActionCard family={p.family} tools={p.tools} />
    {/if}
  {/each}
</div>
