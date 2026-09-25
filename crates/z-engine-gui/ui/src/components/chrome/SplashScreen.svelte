<script lang="ts">
  import { onMount } from "svelte";
  import { prefersReducedMotion } from "$lib/ui/motion";

  /**
   * Drives the boot splash in index.html: it stays until the app has loaded
   * (at least long enough to finish drawing, at most a few seconds), then
   * hands off: the glow flies to the element marked `data-splash-target`
   * (the companion) while the backdrop dissolves into the app beneath.
   */
  type Props = { ready: boolean; onDone: () => void };
  let { ready, onDone }: Props = $props();

  const MIN_MS = 950;
  const MAX_MS = 3500;
  const FLIGHT_MS = 760;
  let timedOut = $state(false);
  let left = false;

  onMount(() => {
    const timer = window.setTimeout(() => (timedOut = true), Math.max(0, MAX_MS - performance.now()));
    return () => window.clearTimeout(timer);
  });

  $effect(() => {
    if (!(ready || timedOut) || left) return;
    const timer = window.setTimeout(leave, Math.max(0, MIN_MS - performance.now()));
    return () => window.clearTimeout(timer);
  });

  function leave() {
    if (left) return;
    left = true;
    const splash = document.getElementById("boot-splash");
    if (!splash) {
      onDone();
      return;
    }
    requestAnimationFrame(() => {
      const target = document.querySelector("[data-splash-target]");
      const glow = splash.querySelector(".splash-glow");
      const reduced = prefersReducedMotion();
      if (target && glow && !reduced) {
        const to = target.getBoundingClientRect();
        const from = glow.getBoundingClientRect();
        splash.style.setProperty("--to-x", `${to.left + to.width / 2 - (from.left + from.width / 2)}px`);
        splash.style.setProperty("--to-y", `${to.top + to.height / 2 - (from.top + from.height / 2)}px`);
        splash.style.setProperty("--to-scale", `${Math.max(0.08, (to.width * 1.4) / from.width)}`);
        splash.classList.add("is-flying");
      }
      splash.classList.add("is-leaving");
      onDone();
      window.setTimeout(() => splash.remove(), reduced ? 240 : FLIGHT_MS);
    });
  }
</script>
