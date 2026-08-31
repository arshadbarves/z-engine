<script lang="ts">
  import "../../splash.css";

  const BOOT_ID = "boot-splash";

  type Props = { onDone: () => void };
  let { onDone }: Props = $props();

  $effect(() => {
    const el = document.getElementById(BOOT_ID);
    if (el) {
      el.remove();
    }
    onDone();
  });
</script>

