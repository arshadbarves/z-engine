export function nextActivityTabIndex(current: number, count: number, key: string): number {
  if (count <= 0) return 0;
  if (key === "Home") return 0;
  if (key === "End") return count - 1;
  if (key === "ArrowLeft") return (current - 1 + count) % count;
  if (key === "ArrowRight") return (current + 1) % count;
  return current;
}

export function resolveActivityTab<T>(
  active: T,
  visible: readonly T[],
  focusWasInTablist: boolean,
): { active: T; restoreFocus: boolean } {
  if (visible.includes(active)) return { active, restoreFocus: false };
  return {
    active: visible[0] ?? active,
    restoreFocus: visible.length > 0 && focusWasInTablist,
  };
}

export function shouldRestoreProcessTrigger(
  wasRunning: boolean,
  isRunning: boolean,
  focusInsideInspector: boolean,
): boolean {
  return wasRunning && !isRunning && focusInsideInspector;
}
