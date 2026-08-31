<script lang="ts">
  import { Tooltip } from "bits-ui";
  import type { Snippet } from "svelte";

  type Props = {
    text: string;
    children: Snippet;
  };

  let { text, children }: Props = $props();
</script>

<Tooltip.Provider>
  <Tooltip.Root delayDuration={280}>
    <Tooltip.Trigger>
      {#snippet child({ props })}
        <span {...props} style="display:inline-flex">
          {@render children()}
        </span>
      {/snippet}
    </Tooltip.Trigger>
    <Tooltip.Content class="tip" sideOffset={6}>{text}</Tooltip.Content>
  </Tooltip.Root>
</Tooltip.Provider>
