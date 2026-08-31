import { useState, useSyncExternalStore } from "react";
import { Check, Copy, Undo2 } from "../lib/icons";
import { abort, revertToTurn } from "../lib/commands";
import { busyStore, draftStore, pushToast, trimTranscript, type Msg } from "../lib/events";

const COLLAPSE_CHARS = 380;
const COLLAPSE_LINES = 6;

/** Premium user prompt bubble with discrete micro-actions positioned below. */
export function UserCard({ m }: { m: Msg }) {
  const busy = useSyncExternalStore(busyStore.subscribe, () => busyStore.getSnapshot());
  const [copied, setCopied] = useState(false);
  const [pending, setPending] = useState(false);
  const lines = m.text.split("\n").length;
  const isLong = m.text.length > COLLAPSE_CHARS || lines > COLLAPSE_LINES;
  const [expanded, setExpanded] = useState(!isLong);

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
    <div className="user-message-row" id={`msg-${m.id}`} data-msg-id={m.id}>
      <div className="user-message-wrapper">
        {/* Main User Prompt Bubble */}
        <div className="user-message-bubble">
          <div className={`user-prompt-text${isLong && !expanded ? " collapsed" : ""}`}>
            {m.text}
          </div>

          {m.images && m.images.length > 0 && (
            <div className="user-attached-images">
              {m.images.map((url, i) => (
                <img key={i} src={url} alt={`attached ${i + 1}`} className="user-img-thumb" />
              ))}
            </div>
          )}

          {isLong && (
            <button
              type="button"
              className="user-expand-btn"
              onClick={() => setExpanded((v) => !v)}
            >
              {expanded ? "Show less" : "Show more"}
            </button>
          )}
        </div>

        {/* Micro-Actions Below the Chat Bubble (Hover Only, Minimalist Icon Buttons) */}
        <div className="user-bubble-actions">
          <button
            type="button"
            className={`bubble-action-icon-btn${copied ? " ok" : ""}`}
            title={copied ? "Copied" : "Copy prompt"}
            onClick={() => void copy()}
            aria-label="Copy prompt"
          >
            {copied ? (
              <Check size={12} strokeWidth={2} className="copy-ok" />
            ) : (
              <Copy size={12} strokeWidth={1.8} />
            )}
          </button>

          {canRevert && (
            <button
              type="button"
              className="bubble-action-icon-btn"
              disabled={pending}
              title={busy ? "Stop & edit prompt" : "Revert & edit prompt"}
              onClick={() => void revert()}
              aria-label="Revert & edit prompt"
            >
              <Undo2 size={12} strokeWidth={1.8} />
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
