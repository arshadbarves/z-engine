import { ChevronDown, ChevronRight, Sparkles } from "../lib/icons";
import { handleEvent, type Msg } from "../lib/events";

/** Thinking stream: live while streaming, auto-collapses to a header
 * the user can click to re-expand the retained body. */
export function ThinkingBlock({ m }: { m: Msg }) {
  const streaming = Boolean(m.streaming);
  const collapsed = Boolean(m.collapsed) && !streaming;
  const chars = m.thinkingBody?.length ?? 0;

  if (streaming) {
    return (
      <div className="msg thinking streaming">
        <span className="thinking-head">
          <span className="reason-pulse-dot" />
          <Sparkles size={12} className="thinking-icon" />
          <span>Thinking… ({chars} chars)</span>
        </span>
      </div>
    );
  }

  return (
    <div className={`msg thinking${collapsed ? "" : " open"}`}>
      <button
        type="button"
        className="thinking-head"
        onClick={() => handleEvent({ type: "toggleThinking", id: m.id })}
        title={collapsed ? "Show thought process" : "Hide thought process"}
      >
        {collapsed ? <ChevronRight size={12} /> : <ChevronDown size={12} />}
        <Sparkles size={12} className="thinking-icon" />
        <span>Thought process ({chars} chars)</span>
      </button>
      {!collapsed && m.thinkingBody && (
        <pre className="thinking-body">{m.thinkingBody}</pre>
      )}
    </div>
  );
}
