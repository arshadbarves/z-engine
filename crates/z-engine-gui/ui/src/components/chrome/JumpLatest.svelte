<script lang="ts">
  import Icon, { ArrowDown } from "$lib/ui/icons";

  type Props = { onJump: () => void };
  let { onJump }: Props = $props();
</script>

<button type="button" class="jump-latest" title="Jump to latest" onclick={onJump}>
  <Icon icon={ArrowDown} size={16} />
</button>
