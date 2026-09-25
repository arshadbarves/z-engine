<script lang="ts" module>
  import { Dialog as Bits } from "bits-ui";
  export const Root = Bits.Root;
  export const Close = Bits.Close;
</script>

<script lang="ts">
  import type { Snippet } from "svelte";

  type Props = {
    title?: string;
    /** Names the dialog for assistive tech when it shows no title. */
    label?: string;
    overlayClass?: string;
    contentClass?: string;
    closing?: boolean;
    children: Snippet;
  };

  let {
    title,
    label,
    overlayClass = "",
    contentClass = "",
    closing = false,
    children,
  }: Props = $props();
</script>

<Bits.Portal>
  <Bits.Overlay class="modal-overlay{closing ? ' is-closing' : ''} {overlayClass}" />
  <Bits.Content class="modal{closing ? ' is-closing' : ''} {contentClass}" aria-label={title ? undefined : label}>
    {#if title}
      <Bits.Title class="modal-head">{title}</Bits.Title>
    {/if}
    {@render children()}
  </Bits.Content>
</Bits.Portal>
