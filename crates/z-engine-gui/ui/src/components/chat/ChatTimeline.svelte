<script lang="ts">
  import type { Msg } from "$lib/types";

  type Props = { messages: Msg[] };
  let { messages }: Props = $props();

  const users = $derived(messages.filter((m) => m.kind === "user"));

  function jumpTo(id: number) {
    const el = document.getElementById(`msg-${id}`);
    const transcript = el?.closest(".transcript");
    if (el && transcript) {
      const tRect = transcript.getBoundingClientRect();
      const elRect = el.getBoundingClientRect();
      const offset = elRect.top - tRect.top + transcript.scrollTop - 20;
      transcript.scrollTo({
        top: Math.max(0, offset),
        behavior: "smooth",
      });
    } else {
      el?.scrollIntoView({ behavior: "smooth", block: "start" });
    }
  }
</script>

{#if users.length >= 2}
  <nav class="chat-timeline" aria-label="Jump to turn">
    {#each users as m, i}
      <button
        type="button"
        class="chat-timeline-tick"
        title={m.text.slice(0, 120) || `Turn ${i + 1}`}
        onclick={() => jumpTo(m.id)}
      >
        <span class="chat-timeline-dot"></span>
        <span class="chat-timeline-n">{i + 1}</span>
      </button>
    {/each}
  </nav>
{/if}
