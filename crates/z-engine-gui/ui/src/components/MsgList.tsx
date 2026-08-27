import { useEffect, useState } from "react";
import { AssistantMarkdown } from "./Markdown";
import { ToolCard } from "./ToolCard";
import { ApprovalCard } from "./ApprovalCard";
import { ThinkingBlock } from "./ThinkingBlock";
import { UserCard } from "./UserCard";
import { LogoMark } from "./LogoMark";
import { draftStore, type Msg } from "../lib/events";

export const HERO_EXAMPLES = [
  "Fix the failing tests",
  "Explain this codebase in 5 bullets",
  "Add input validation to the CLI",
];

function WorkingRow() {
  const [secs, setSecs] = useState(0);
  useEffect(() => {
    const t = setInterval(() => setSecs((s) => s + 1), 1000);
    return () => clearInterval(t);
  }, []);
  return (
    <div className="msg working" aria-live="polite">
      <span className="working-glyph">✻</span> working… <span className="working-sec">{secs}s</span>
      <span className="working-hint">Esc aborts</span>
    </div>
  );
}

function MsgCard({
  m,
  onApprove,
  onDeny,
}: {
  m: Msg;
  onApprove: (m: Msg, decision: "once" | "session" | "persist") => void;
  onDeny: (m: Msg) => void;
}) {
  if (m.kind === "approval") {
    return (
      <ApprovalCard m={m} onApprove={(d) => onApprove(m, d)} onDeny={() => onDeny(m)} />
    );
  }
  if (m.kind === "user") return <UserCard m={m} />;
  if (m.kind === "assistant") {
    return (
      <div className={`msg assistant${m.streaming ? " streaming" : ""}`}>
        <AssistantMarkdown text={m.text} />
      </div>
    );
  }
  if (m.kind === "command") {
    return (
      <div className="msg command">
        <span className="cmd-glyph">!</span>
        <span>{m.text.replace(/^!\s*/, "")}</span>
      </div>
    );
  }
  if (m.kind === "tool") return <ToolCard m={m} />;
  if (m.kind === "thinking") return <ThinkingBlock m={m} />;
  if (m.kind === "notice" && m.text.startsWith("$ ")) {
    return <div className="msg notice shell">{m.text}</div>;
  }
  return <div className={`msg ${m.kind}`}>{m.text}</div>;
}

export function MsgList({
  messages,
  busy,
  projectName,
  onApprove,
  onDeny,
}: {
  messages: Msg[];
  busy: boolean;
  projectName: string | null;
  onApprove: (m: Msg, decision: "once" | "session" | "persist") => void;
  onDeny: (m: Msg) => void;
}) {
  return (
    <div className="transcript-inner">
      {messages.length === 0 && (
        <div className="hero">
          <LogoMark size={28} />
          <h1>What should we build{projectName ? ` in ${projectName}` : ""}?</h1>
          <p>
            Describe a task — Z Engine reads files, runs commands, edits code and verifies the
            result before reporting back.
          </p>
          <div className="hero-chips">
            {HERO_EXAMPLES.map((ex) => (
              <button key={ex} className="chip" onClick={() => draftStore.set(ex)}>
                {ex}
              </button>
            ))}
          </div>
        </div>
      )}
      {messages.map((m) => (
        <MsgCard
          key={m.id}
          m={m}
          onApprove={onApprove}
          onDeny={onDeny}
        />
      ))}
      {busy && <WorkingRow key={`working-${messages.length}`} />}
    </div>
  );
}
