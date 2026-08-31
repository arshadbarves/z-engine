import { draftStore } from "./events";

let history: string[] = [];
let histPos: number | null = null;

/** Svelte-friendly composer history (replaces the React hook). */
export function createComposerHistory() {
  function pushHistory(item: string) {
    if (!item.trim()) return;
    history = [...history, item].slice(-100);
    histPos = null;
  }

  function historyPrev(setCaret: (n: number) => void) {
    if (history.length === 0) return;
    const pos = histPos === null ? history.length - 1 : Math.max(0, histPos - 1);
    histPos = pos;
    draftStore.set(history[pos]);
    setCaret(history[pos].length);
  }

  function historyNext(setCaret: (n: number) => void) {
    if (histPos === null) return;
    if (histPos + 1 >= history.length) {
      histPos = null;
      draftStore.set("");
      setCaret(0);
    } else {
      histPos += 1;
      draftStore.set(history[histPos]);
      setCaret(history[histPos].length);
    }
  }

  return { pushHistory, historyPrev, historyNext };
}

/** @deprecated React hook kept for the old Composer until it is deleted. */
export function useComposerHistory() {
  return createComposerHistory();
}
