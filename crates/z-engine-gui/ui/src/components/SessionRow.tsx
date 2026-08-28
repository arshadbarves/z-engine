import { AlertCircle, LoaderCircle, MessageSquare, Trash2 } from "lucide-react";
import type { SessionActivity } from "../lib/events";
import { fmtHumanRelTime, type SessionEntry } from "../lib/util";
import { sessionLabel } from "../lib/sessionList";

export function SessionRow({
  s,
  active,
  state,
  onOpen,
  onDelete,
}: {
  s: SessionEntry;
  active: boolean;
  state: SessionActivity | null;
  onOpen: (path: string, projectRoot?: string | null) => void;
  onDelete: (path: string) => void;
}) {
  const unread =
    !active && !state && (s.unreadOutcome === "completed" || s.unreadOutcome === "aborted")
      ? s.unreadOutcome
      : null;

  const rawTitle = sessionLabel(s.firstUserMsg);
  // Human-friendly title fallback if title is just a single number
  const title = /^\d+$/.test(rawTitle) ? `Conversation #${rawTitle}` : rawTitle;
  const timeStr = fmtHumanRelTime(Number(s.modifiedMs));

  return (
    <div
      className={`session${active ? " active" : ""}${state ? ` ${state}` : ""}${
        unread ? ` unread unread-${unread}` : ""
      }`}
      role="button"
      tabIndex={0}
      title={
        state === "approval"
          ? `Approval needed — ${title}`
          : unread
            ? `${unread === "aborted" ? "Aborted" : "Completed"} — ${title}`
            : `${title} (${timeStr})`
      }
      onClick={(e) => {
        e.stopPropagation();
        onOpen(s.path, s.projectRoot);
      }}
      onKeyDown={(e) => e.key === "Enter" && onOpen(s.path, s.projectRoot)}
    >
      <div className="sess-icon-col">
        {state === "working" ? (
          <LoaderCircle size={13} className="spin sess-working-icon" />
        ) : state === "approval" ? (
          <AlertCircle size={13} className="sess-approval-icon" />
        ) : (
          <MessageSquare size={13} className="sess-icon" />
        )}
      </div>

      <div className="sess-content">
        <div className="sess-title-row">
          <span className="sess-preview">{title}</span>
        </div>
        <div className="sess-meta-row">
          <span className="sess-time">{timeStr}</span>
          {state === "working" && <span className="sess-badge working">Thinking…</span>}
          {state === "approval" && <span className="sess-badge approval">Approval</span>}
        </div>
      </div>

      <span className="sess-tail">
        {unread && (
          <span
            className="sess-unread-dot"
            role="status"
            aria-label={`${unread} — unopened`}
          />
        )}
        <button
          className="del"
          title="Delete conversation"
          onClick={(e) => {
            e.stopPropagation();
            onDelete(s.path);
          }}
        >
          <Trash2 size={12} />
        </button>
      </span>
    </div>
  );
}
