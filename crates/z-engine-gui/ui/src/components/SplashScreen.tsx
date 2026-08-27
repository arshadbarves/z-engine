import { useEffect, useState } from "react";
import { LogoMark } from "./LogoMark";
import "../splash.css";

const SPLASH_MS = 1400;

export function SplashScreen({ onDone }: { onDone: () => void }) {
  const [leaving, setLeaving] = useState(false);

  useEffect(() => {
    const reduce =
      typeof window !== "undefined" &&
      window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    if (reduce) {
      onDone();
      return;
    }
    const leaveAt = window.setTimeout(() => setLeaving(true), SPLASH_MS);
    const doneAt = window.setTimeout(() => onDone(), SPLASH_MS + 320);
    return () => {
      window.clearTimeout(leaveAt);
      window.clearTimeout(doneAt);
    };
  }, [onDone]);

  return (
    <div className={`splash${leaving ? " leaving" : ""}`} role="status" aria-label="Z Engine">
      <div className="splash-inner">
        <LogoMark size={52} className="splash-mark" />
        <p className="splash-word">Z Engine</p>
      </div>
    </div>
  );
}
