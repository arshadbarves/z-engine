<script lang="ts">
  import Markdown from "./Markdown.svelte";
  import MessageActions from "./MessageActions.svelte";

  type Props = { text: string; streaming?: boolean };
  let { text, streaming = false }: Props = $props();
</script>

<div class="assistant-block">
  <div class={`msg assistant${streaming ? " streaming" : ""}`}>
    <Markdown {text} />
  </div>
  {#if !streaming && text.trim()}
    <MessageActions {text} />
  {/if}
</div>
