import { useRef } from "react";
import { draftStore } from "./events";

export function useComposerHistory() {
  const historyRef = useRef<string[]>([]);
  const histPosRef = useRef<number | null>(null);

  function pushHistory(item: string) {
    if (!item.trim()) return;
    historyRef.current = [...historyRef.current, item].slice(-100);
    histPosRef.current = null;
  }

  function historyPrev(setCaret: (n: number) => void) {
    const h = historyRef.current;
    if (h.length === 0) return;
    const pos =
      histPosRef.current === null ? h.length - 1 : Math.max(0, histPosRef.current - 1);
    histPosRef.current = pos;
    draftStore.set(h[pos]);
    setCaret(h[pos].length);
  }

  function historyNext(setCaret: (n: number) => void) {
    const h = historyRef.current;
    const pos = histPosRef.current;
    if (pos === null) return;
    if (pos + 1 >= h.length) {
      histPosRef.current = null;
      draftStore.set("");
      setCaret(0);
    } else {
      histPosRef.current = pos + 1;
      draftStore.set(h[pos + 1]);
      setCaret(h[pos + 1].length);
    }
  }

  return { pushHistory, historyPrev, historyNext, histPosRef };
}
