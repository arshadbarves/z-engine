<script lang="ts">
  import { Spring } from "svelte/motion";
  import type { CompanionPose, Gaze } from "$lib/domain/companion";
  import { SPRING, prefersReducedMotion } from "$lib/ui/motion";

  /**
   * The companion: a glass orb whose eyes and posture show the agent's
   * state. Purely decorative; the status line carries the meaning.
   */
  type Props = {
    pose: CompanionPose;
    /** Plan progress drawn as a ring around the orb, 0..1. */
    progress: number | null;
    /** Running agents and jobs, drawn as orbiting sprites (at most four). */
    helpers: number;
    /** Drawn size in pixels; the title bar uses 30. */
    size?: number;
  };
  let { pose, progress, helpers, size = 30 }: Props = $props();

  const uid = $props.id();
  const RING = 2 * Math.PI * 13;
  const GAZE: Record<Exclude<Gaze, "pointer" | "scan">, [number, number]> = {
    center: [0, 0],
    down: [0, 1.5],
    up: [0, -1.7],
    left: [-2, 0.3],
  };

  let root: SVGSVGElement | undefined = $state();
  let blinking = $state(false);
  const gaze = new Spring({ x: 0, y: 0 }, SPRING.snappy);
  const sprites = $derived(Array.from({ length: Math.min(4, helpers) }, (_, i) => i));

  $effect(() => {
    const mode = pose.gaze;
    if (mode === "pointer") return;
    const [x, y] = mode === "scan" ? [0, 0] : GAZE[mode];
    void gaze.set({ x, y }, { instant: prefersReducedMotion() });
  });

  $effect(() => {
    if (pose.gaze !== "pointer" || !root) return;
    const el = root;
    const onMove = (e: PointerEvent) => {
      const box = el.getBoundingClientRect();
      const dx = e.clientX - (box.left + box.width / 2);
      const dy = e.clientY - (box.top + box.height / 2);
      const dist = Math.hypot(dx, dy) || 1;
      const reach = Math.min(1, dist / 36);
      void gaze.set({ x: (dx / dist) * 2 * reach, y: (dy / dist) * 1.8 * reach });
    };
    window.addEventListener("pointermove", onMove, { passive: true });
    return () => window.removeEventListener("pointermove", onMove);
  });

  $effect(() => {
    if (prefersReducedMotion()) return;
    let timer = 0;
    const schedule = () => {
      timer = window.setTimeout(
        () => {
          blinking = true;
          timer = window.setTimeout(() => {
            blinking = false;
            schedule();
          }, 130);
        },
        2600 + Math.random() * 3400,
      );
    };
    schedule();
    return () => window.clearTimeout(timer);
  });
</script>

<svg
  bind:this={root}
  class={`companion mood-${pose.mood} tone-${pose.tone} gaze-${pose.gaze}${blinking ? " is-blinking" : ""}`}
  width={size}
  height={size}
  viewBox="0 0 30 30"
  aria-hidden="true"
  style:--gx={gaze.current.x}
  style:--gy={gaze.current.y}
>
  <defs>
    <radialGradient id={`${uid}-body`} cx="38%" cy="30%" r="80%">
      <stop offset="0%" class="orb-stop-light" />
      <stop offset="55%" class="orb-stop-mid" />
      <stop offset="100%" class="orb-stop-edge" />
    </radialGradient>
    <filter id={`${uid}-glow`} x="-60%" y="-60%" width="220%" height="220%">
      <feGaussianBlur stdDeviation="2.4" />
    </filter>
  </defs>

  {#if progress !== null}
    <circle class="orb-ring-track" cx="15" cy="15" r="13" />
    <circle
      class="orb-ring"
      cx="15"
      cy="15"
      r="13"
      stroke-dasharray={RING}
      stroke-dashoffset={RING * (1 - Math.min(1, Math.max(0, progress)))}
    />
  {/if}

  {#each sprites as i (i)}
    <circle class="orb-sprite" cx="15" cy="2.2" r="1.5" style:--i={i} />
  {/each}

  <g class="orb-body">
    <circle class="orb-aura" cx="15" cy="15" r="10" filter={`url(#${uid}-glow)`} />
    <circle cx="15" cy="15" r="10" fill={`url(#${uid}-body)`} />
    <circle class="orb-rim" cx="15" cy="15" r="9.7" />
    <ellipse class="orb-shine" cx="11.6" cy="10.2" rx="3.2" ry="1.8" transform="rotate(-28 11.6 10.2)" />
    <g class="orb-eyes">
      <ellipse class="orb-eye" cx="12" cy="15.6" rx="1.3" ry="1.95" />
      <ellipse class="orb-eye" cx="18" cy="15.6" rx="1.3" ry="1.95" />
      <path class="orb-eye-happy" d="M10.7 16.3 q1.3 -1.9 2.6 0 M16.7 16.3 q1.3 -1.9 2.6 0" />
    </g>
  </g>

  {#if pose.particles === "thought"}
    <g class="orb-thought">
      <circle cx="23.2" cy="7.6" r="0.9" />
      <circle cx="25.6" cy="5.2" r="1.15" />
      <circle cx="27.8" cy="2.6" r="1.4" />
    </g>
  {:else if pose.particles === "sparkles"}
    <g class="orb-sparkles">
      <path d="M4 7 l.7 1.6 1.6.7 -1.6.7 -.7 1.6 -.7 -1.6 -1.6 -.7 1.6 -.7z" />
      <path d="M25 5 l.6 1.3 1.3.6 -1.3.6 -.6 1.3 -.6 -1.3 -1.3 -.6 1.3 -.6z" />
      <path d="M25.5 22 l.5 1.1 1.1.5 -1.1.5 -.5 1.1 -.5 -1.1 -1.1 -.5 1.1 -.5z" />
    </g>
  {:else if pose.particles === "sleep"}
    <text class="orb-z" x="22.5" y="8.5">z</text>
  {/if}
</svg>
