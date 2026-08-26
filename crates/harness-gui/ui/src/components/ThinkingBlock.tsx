import { handleEvent, type Msg } from "../lib/events";

/** Thinking stream: live while streaming, auto-collapses to a header
 * the user can click to re-expand the retained body. */
export function ThinkingBlock({ m }: { m: Msg }) {
  const streaming = Boolean(m.streaming);
  const collapsed = Boolean(m.collapsed) && !streaming;

  if (streaming) {
    return (
      <div className="msg thinking streaming">
        <span className="thinking-head">✻ thinking… ({m.thinkingBody?.length ?? 0} chars)</span>
      </div>
    );
  }

  const n = m.thinkingBody?.length ?? 0;
  return (
    <div className={`msg thinking${collapsed ? "" : " open"}`}>
      <button
        className="thinking-head"
        onClick={() => handleEvent({ type: "toggleThinking", id: m.id })}
        title={collapsed ? "Show thought" : "Hide thought"}
      >
        ✻ thought ({n} chars)
      </button>
      {!collapsed && m.thinkingBody && (
        <pre className="thinking-body">{m.thinkingBody}</pre>
      )}
    </div>
  );
}
