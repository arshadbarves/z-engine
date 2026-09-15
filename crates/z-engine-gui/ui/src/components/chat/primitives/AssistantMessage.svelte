<script lang="ts">
  import type { Msg } from "$lib/types";
  import Markdown from "../Markdown.svelte";
  import MessageActions from "./MessageActions.svelte";
  import ToolActivityGroup from "./ToolActivityGroup.svelte";

  type Props = { message: Msg; workItems?: Msg[] };
  let { message, workItems = [] }: Props = $props();

  let processExpanded = $state(false);
  const hasText = $derived(message.text.trim().length > 0);
</script>

<article class="assistant-turn" id={`msg-${message.id}`} data-msg-id={message.id}>
  {#if workItems.length > 0}
    <ToolActivityGroup items={workItems} bind:expanded={processExpanded} />
  {/if}

  {#if hasText || message.streaming}
    <div class={`msg assistant${message.streaming ? " streaming" : ""}`}>
      <Markdown text={message.text} />
    </div>
  {/if}

  {#if hasText && !message.streaming}
    <MessageActions
      text={message.text}
      onInspect={workItems.length > 0 ? () => (processExpanded = !processExpanded) : undefined}
      inspecting={processExpanded}
    />
  {/if}
</article>
