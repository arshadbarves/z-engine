<script lang="ts">
  import type { Msg } from "$lib/types";

  type Props = { messages: Msg[] };
  let { messages }: Props = $props();

  const users = $derived(messages.filter((m) => m.kind === "user"));
  let hoveredId = $state<number | null>(null);

  function jumpTo(id: number) {
    const el = document.getElementById(`msg-${id}`);
    const transcript = el?.closest(".transcript");
    if (el && transcript) {
      const tRect = transcript.getBoundingClientRect();
      const elRect = el.getBoundingClientRect();
      const offset = elRect.top - tRect.top + transcript.scrollTop - 24;
      transcript.scrollTo({
        top: Math.max(0, offset),
        behavior: "smooth",
      });
    } else {
      el?.scrollIntoView({ behavior: "smooth", block: "start" });
    }
  }

  function getSnippet(text: string): string {
    const clean = text.replace(/[\n\r]+/g, " ").trim();
    return clean.length > 55 ? `${clean.slice(0, 52)}…` : clean;
  }
</script>

{#if users.length >= 2}
  <nav class="chat-timeline-rail" aria-label="Jump to conversation prompt">
    <div class="chat-timeline-track">
      {#each users as m (m.id)}
        <div class="chat-timeline-node">
          <button
            type="button"
            class="chat-timeline-pill"
            aria-label={`Jump to: ${getSnippet(m.text)}`}
            onclick={() => jumpTo(m.id)}
            onmouseenter={() => (hoveredId = m.id)}
            onmouseleave={() => {
              if (hoveredId === m.id) hoveredId = null;
            }}
            onfocus={() => (hoveredId = m.id)}
            onblur={() => {
              if (hoveredId === m.id) hoveredId = null;
            }}
          >
            <span class="chat-timeline-core"></span>
          </button>

          {#if hoveredId === m.id}
            <div class="chat-timeline-tip" role="tooltip">
              <span class="tip-text">{getSnippet(m.text) || "Jump to prompt"}</span>
            </div>
          {/if}
        </div>
      {/each}
    </div>
  </nav>
{/if}

