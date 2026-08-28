import { useState } from "react";
import { ChevronRight } from "lucide-react";
import { splitWork } from "../lib/toolGroups";
import type { Msg } from "../lib/events";
import { ActionCard } from "./ActionCard";

function ReasonLine({ msg, text }: { msg: Msg; text: string }) {
  const [open, setOpen] = useState(false);
  const body = msg.thinkingBody;
  const isStreaming = Boolean(msg.streaming);
  return (
    <div className={`reason-line${isStreaming ? " streaming" : ""}`}>
      <button
        type="button"
        className="reason-btn"
        onClick={() => body && setOpen((v) => !v)}
        disabled={!body && !isStreaming}
      >
        <span className={`reason-chevron${open ? " open" : ""}`}>
          <ChevronRight size={11} />
        </span>
        <span className="reason-label">
          {isStreaming && <span className="reason-pulse-dot" aria-hidden />}
          Reasoning
        </span>
        <span className="reason-text">{text || (isStreaming ? "thinking…" : "")}</span>
      </button>
      {open && body && <pre className="thinking-body">{body}</pre>}
    </div>
  );
}

/** Reasoning line + compact action cards for a turn's work. */
export function ActivityStrip({ items }: { items: Msg[] }) {
  const parts = splitWork(items);
  return (
    <div className="activity-strip">
      {parts.map((p) =>
        p.type === "reason" ? (
          <ReasonLine key={p.msg.id} msg={p.msg} text={p.text} />
        ) : (
          <ActionCard key={p.tools[0].id} family={p.family} tools={p.tools} />
        ),
      )}
    </div>
  );
}
