import { useEffect, useRef, useSyncExternalStore } from "react";
import { hideShell, shellStore } from "../lib/shellStore";
import { X } from "../lib/icons";

/** Quiet terminal panel for `!` passthrough — never enters the chat. */
export function ShellOverlay() {
  const { visible, entries } = useSyncExternalStore(
    shellStore.subscribe,
    () => shellStore.getSnapshot(),
  );
  const scroller = useRef<HTMLPreElement>(null);

  useEffect(() => {
    const el = scroller.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [entries]);

  if (!visible || entries.length === 0) return null;

  const last = entries[entries.length - 1];
  const body = entries
    .slice(-6)
    .map((e) => (e.cmd ? `$ ${e.cmd}\n${e.lines.join("\n")}` : e.lines.join("\n")))
    .join("\n\n");

  return (
    <div className="term-panel" role="log" aria-label="Shell output">
      <div className="term-head">
        <span className="term-prompt">$</span>
        <span className="term-cmd" title={last.cmd}>
          {last.cmd || "shell"}
        </span>
        <button type="button" className="mini" title="Hide (Esc)" onClick={hideShell}>
          <X size={12} />
        </button>
      </div>
      <pre ref={scroller} className="term-body">
        {body || "running…"}
      </pre>
    </div>
  );
}
