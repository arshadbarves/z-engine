import { useEffect, useState } from "react";

/**
 * Standard presence lifecycle hook for fluid, physics-based entrance and exit transitions.
 * Keeps components mounted until the exit animation completes.
 */
export function usePresence(isOpen: boolean, exitDurationMs = 280) {
  const [mounted, setMounted] = useState(isOpen);
  const [isClosing, setIsClosing] = useState(false);

  useEffect(() => {
    let timeoutId: number | undefined;

    if (isOpen) {
      setMounted(true);
      setIsClosing(false);
    } else if (mounted) {
      setIsClosing(true);
      timeoutId = window.setTimeout(() => {
        setMounted(false);
        setIsClosing(false);
      }, exitDurationMs);
    }

    return () => {
      if (timeoutId) clearTimeout(timeoutId);
    };
  }, [isOpen, exitDurationMs, mounted]);

  return { mounted, isClosing };
}
