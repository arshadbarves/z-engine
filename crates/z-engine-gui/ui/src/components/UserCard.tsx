import { useState, useSyncExternalStore } from "react";
import { Check, Copy, Undo2 } from "lucide-react";
import { abort, revertToTurn } from "../lib/commands";
import { busyStore, draftStore, pushToast, trimTranscript, type Msg } from "../lib/events";

const COLLAPSE_CHARS = 280;
const COLLAPSE_LINES = 4;

/** User bubble with copy / edit-revert sitting under the bar. */
export function UserCard({ m }: { m: Msg }) {
  const busy = useSyncExternalStore(busyStore.subscribe, () => busyStore.getSnapshot());
  const [copied, setCopied] = useState(false);
  const [pending, setPending] = useState(false);
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

  async function revert() {
    if (!canRevert || pending) return;
    setPending(true);
    try {
      // Abort first so RevertToTurn is not dropped mid-stream.
      if (busy) await abort();
      draftStore.set(m.text);
      trimTranscript(m.runTurn as number);
      await revertToTurn(m.runTurn as number);
    } catch (e) {
      console.error(e);
      pushToast("Could not restore that prompt", "warn");
    } finally {
      setPending(false);
    }
  }

  return (
    <div className="user-wrap" id={`msg-${m.id}`} data-msg-id={m.id}>
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
          disabled={!canRevert || pending}
          title={
            canRevert
              ? busy
                ? "Stop this turn and move the prompt back to the composer"
                : "Move this prompt back to the composer"
              : "This prompt cannot be restored"
          }
          onClick={() => void revert()}
        >
          <Undo2 size={12} />
          <span>{busy ? "Edit" : "Revert"}</span>
        </button>
      </div>
    </div>
  );
}
