<script lang="ts">
  import "../../splash.css";

  const HOLD_MS = 1100;
  const EXIT_MS = 420;
  const BOOT_ID = "boot-splash";

  type Props = { onDone: () => void };
  let { onDone }: Props = $props();

  $effect(() => {
    const el = document.getElementById(BOOT_ID);
    const reduce =
      typeof window !== "undefined" &&
      window.matchMedia("(prefers-reduced-motion: reduce)").matches;

    if (!el) {
      onDone();
      return;
    }
    if (reduce) {
      el.remove();
      onDone();
      return;
    }

    // Enhance the static HTML splash with wordmark + bar classes already in markup.
    el.classList.add("splash-live");
    const leaveAt = window.setTimeout(() => el.classList.add("leaving"), HOLD_MS);
    const doneAt = window.setTimeout(() => {
      el.remove();
      onDone();
    }, HOLD_MS + EXIT_MS);
    return () => {
      window.clearTimeout(leaveAt);
      window.clearTimeout(doneAt);
    };
  });
</script>
