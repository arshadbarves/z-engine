import { useState, useSyncExternalStore } from "react";
import { Check, Copy, Undo2 } from "lucide-react";
import { busyStore, draftStore, pushToast, trimTranscript, type Msg } from "../lib/events";
import { revertToTurn } from "../lib/commands";

/** User bubble with copy + per-message revert (files + transcript). */
export function UserCard({ m }: { m: Msg }) {
  const busy = useSyncExternalStore(busyStore.subscribe, () => busyStore.getSnapshot());
  const [copied, setCopied] = useState(false);

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
    <div className="msg user">
      <div className="msg-actions">
        <button type="button" title={copied ? "Copied" : "Copy message"} onClick={() => void copy()}>
          {copied ? <Check size={12} /> : <Copy size={12} />}
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
        </button>
      </div>
      <div className="user-bubble">
        {m.text}
        {m.images && m.images.length > 0 && (
          <span className="msg-images">
            {m.images.map((url, i) => (
              <img key={i} src={url} alt={`attached ${i + 1}`} />
            ))}
          </span>
        )}
      </div>
    </div>
  );
}
