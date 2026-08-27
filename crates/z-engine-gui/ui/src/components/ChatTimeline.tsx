import type { Msg } from "../lib/events";

function jumpTo(id: number) {
  document.getElementById(`msg-${id}`)?.scrollIntoView({
    behavior: "smooth",
    block: "start",
  });
}

/** Thin side rail: click a tick to jump to that user turn. */
export function ChatTimeline({ messages }: { messages: Msg[] }) {
  const users = messages.filter((m) => m.kind === "user");
  if (users.length < 2) return null;
  return (
    <nav className="chat-timeline" aria-label="Jump to turn">
      {users.map((m, i) => (
        <button
          key={m.id}
          type="button"
          className="chat-timeline-tick"
          title={m.text.slice(0, 120) || `Turn ${i + 1}`}
          onClick={() => jumpTo(m.id)}
        >
          <span className="chat-timeline-dot" />
          <span className="chat-timeline-n">{i + 1}</span>
        </button>
      ))}
    </nav>
  );
}
