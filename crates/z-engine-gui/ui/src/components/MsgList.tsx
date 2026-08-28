import { useEffect, useState, useSyncExternalStore } from "react";
import { AssistantMarkdown } from "./Markdown";
import { ApprovalCard } from "./ApprovalCard";
import { UserCard } from "./UserCard";
import { ActivityStrip } from "./ActivityStrip";
import { LogoMark } from "./LogoMark";
import { draftStore, hydrateStore, type Msg } from "../lib/events";
import { groupTranscript } from "../lib/activity";
import { ChatTimeline } from "./ChatTimeline";
import { HERO_EXAMPLES } from "../lib/constants";

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
  if (m.kind === "error") return <div className="msg error">{m.text}</div>;
  if (m.kind === "status") {
    return (
      <div className={`msg working${m.ok === false ? " aborted" : " done"}`}>
        {m.text}
      </div>
    );
  }
  return null;
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
  const hydrating = useSyncExternalStore(
    hydrateStore.subscribe,
    () => hydrateStore.getSnapshot(),
  );
  const blocks = groupTranscript(messages);
  const streaming = messages.some(
    (m) => m.streaming && (m.kind === "assistant" || m.kind === "thinking" || m.kind === "tool"),
  );
  const showWorking = busy && !streaming && !hydrating;

  return (
    <div className="transcript-stage">
      <ChatTimeline messages={messages} />
      <div className="transcript-inner">
      {messages.length === 0 && !hydrating && (
        <div className="hero">
          <div className="hero-icon-wrap">
            <LogoMark size={26} />
          </div>
          <h1>What should we build{projectName ? ` in ${projectName}` : ""}?</h1>
          <p>
            Describe a task — Z Engine reads files, runs commands, edits code and verifies the
            result before reporting back.
          </p>
          <div className="hero-chips">
            {HERO_EXAMPLES.map((ex) => (
              <button key={ex} className="chip" onClick={() => draftStore.set(ex)} type="button">
                {ex}
              </button>
            ))}
          </div>
        </div>
      )}
      {blocks.map((b) =>
        b.type === "work" ? (
          <ActivityStrip key={b.items[0].id} items={b.items} />
        ) : (
          <MsgCard
            key={b.msg.id}
            m={b.msg}
            onApprove={onApprove}
            onDeny={onDeny}
          />
        ),
      )}
      {showWorking && <WorkingRow key={`working-${messages.length}`} />}
      </div>
    </div>
  );
}
