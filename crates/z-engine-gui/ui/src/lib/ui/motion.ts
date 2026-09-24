import { cubicOut } from "svelte/easing";
import type { TransitionConfig } from "svelte/transition";

/** Spring options for `Spring` from `svelte/motion`, shared so every morph feels alike. */
export const SPRING = {
  snappy: { stiffness: 0.2, damping: 0.62 },
  smooth: { stiffness: 0.14, damping: 0.86 },
} as const;

export function prefersReducedMotion(): boolean {
  return typeof matchMedia === "function" && matchMedia("(prefers-reduced-motion: reduce)").matches;
}

type BlurFadeParams = { delay?: number; duration?: number; blur?: number; scale?: number; y?: number };

/** Focus pull: content sharpens in and softens out. Only fades under Reduce Motion. */
export function blurFade(
  _node: Element,
  { delay = 0, duration = 260, blur = 8, scale = 0.98, y = 0 }: BlurFadeParams = {},
): TransitionConfig {
  if (prefersReducedMotion()) return { delay, duration: 120, css: (t) => `opacity: ${t}` };
  return {
    delay,
    duration,
    easing: cubicOut,
    css: (t, u) =>
      `opacity: ${t}; filter: blur(${(u * blur).toFixed(2)}px); scale: ${1 - u * (1 - scale)}; translate: 0 ${u * y}px;`,
  };
}
