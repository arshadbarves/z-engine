<script lang="ts" module>
  import { Dialog as Bits } from "bits-ui";
  export { Bits as Root };
</script>

<script lang="ts">
  import type { Snippet } from "svelte";

  type Props = {
    title?: string;
    overlayClass?: string;
    contentClass?: string;
    closing?: boolean;
    children: Snippet;
  };

  let {
    title,
    overlayClass = "",
    contentClass = "",
    closing = false,
    children,
  }: Props = $props();
</script>

<Bits.Portal>
  <Bits.Overlay class="modal-overlay{closing ? ' is-closing' : ''} {overlayClass}" />
  <Bits.Content class="modal{closing ? ' is-closing' : ''} {contentClass}">
    {#if title}
      <Bits.Title class="modal-head">{title}</Bits.Title>
    {/if}
    {@render children()}
  </Bits.Content>
</Bits.Portal>
