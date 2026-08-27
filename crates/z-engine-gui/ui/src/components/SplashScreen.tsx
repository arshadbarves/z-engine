import { useEffect } from "react";
import "../splash.css";

const SPLASH_MS = 1400;
const BOOT_ID = "boot-splash";

/** Drives the HTML boot splash (in index.html) through its leave
 * animation. Does not mount a second splash — the first paint already
 * is the splash. */
export function SplashScreen({ onDone }: { onDone: () => void }) {
  useEffect(() => {
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

    const leaveAt = window.setTimeout(() => el.classList.add("leaving"), SPLASH_MS);
    const doneAt = window.setTimeout(() => {
      el.remove();
      onDone();
    }, SPLASH_MS + 320);
    return () => {
      window.clearTimeout(leaveAt);
      window.clearTimeout(doneAt);
    };
  }, [onDone]);

  return null;
}
