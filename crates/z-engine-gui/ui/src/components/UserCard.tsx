import { useState, useSyncExternalStore } from "react";
import { Check, Copy, Undo2 } from "lucide-react";
import { busyStore, draftStore, pushToast, trimTranscript, type Msg } from "../lib/events";
import { revertToTurn } from "../lib/commands";

const COLLAPSE_CHARS = 280;
const COLLAPSE_LINES = 4;

/** User bubble with copy/revert sitting under the bar, not inside it. */
export function UserCard({ m, sticky }: { m: Msg; sticky?: boolean }) {
  const busy = useSyncExternalStore(busyStore.subscribe, () => busyStore.getSnapshot());
  const [copied, setCopied] = useState(false);
  const lines = m.text.split("\n").length;
  const long = m.text.length > COLLAPSE_CHARS || lines > COLLAPSE_LINES;
  const [expanded, setExpanded] = useState(!long);

  async function copy() {
    try {
      await navigator.clipboard.writeText(m.text);
      setCopied(true);
      setTimeout(() => setCopied(false), 1200);
    } catch {
      pushToast("Copy failed", "warn");
    }
  }

  const canRevert = typeof m.runTurn === "number";

  function revert() {
    if (!canRevert) return;
    draftStore.set(m.text);
    trimTranscript(m.runTurn as number);
    void revertToTurn(m.runTurn as number);
  }

  return (
    <div className={`user-wrap${sticky ? " sticky" : ""}`}>
      <div className="msg user">
        <div className={`user-bubble${long && !expanded ? " collapsed" : ""}`}>
          {m.text}
          {m.images && m.images.length > 0 && (
            <span className="msg-images">
              {m.images.map((url, i) => (
                <img key={i} src={url} alt={`attached ${i + 1}`} />
              ))}
            </span>
          )}
        </div>
        {long && (
          <button type="button" className="user-more" onClick={() => setExpanded((v) => !v)}>
            {expanded ? "Show less" : "Show more"}
          </button>
        )}
      </div>
      <div className="msg-actions">
        <button type="button" title={copied ? "Copied" : "Copy"} onClick={() => void copy()}>
          {copied ? <Check size={12} /> : <Copy size={12} />}
          <span>{copied ? "Copied" : "Copy"}</span>
        </button>
        <button
          type="button"
          disabled={!canRevert || busy}
          title={
            canRevert
              ? "Restore files and move this prompt back to the composer"
              : "Checkpoint unavailable (message predates this app launch)"
          }
          onClick={() => revert()}
        >
          <Undo2 size={12} />
          <span>Revert</span>
        </button>
      </div>
    </div>
  );
}
