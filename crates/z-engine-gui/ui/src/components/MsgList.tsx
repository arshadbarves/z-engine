import { useEffect, useState, useSyncExternalStore } from "react";
import { AssistantMarkdown } from "./Markdown";
import { ApprovalCard } from "./ApprovalCard";
import { UserCard } from "./UserCard";
import { ActivityStrip } from "./ActivityStrip";
import { LogoMark } from "./LogoMark";
import { draftStore, hydrateStore, type Msg } from "../lib/events";
import { groupTranscript } from "../lib/activity";
import { ChatTimeline } from "./ChatTimeline";
import { HERO_STARTERS } from "../lib/constants";
import { FolderGit2, Search, Sparkles, Workflow, Wrench } from "../lib/icons";

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

function StarterCardIcon({ name }: { name: string }) {
  switch (name) {
    case "Search":
      return <Search size={14} strokeWidth={1.8} />;
    case "Sparkles":
      return <Sparkles size={14} strokeWidth={1.8} />;
    case "Wrench":
      return <Wrench size={14} strokeWidth={1.8} />;
    case "Workflow":
      return <Workflow size={14} strokeWidth={1.8} />;
    default:
      return <Sparkles size={14} strokeWidth={1.8} />;
  }
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
          <div className="start-hub">
            {/* Center Brand Aura */}
            <div className="start-hub-brand">
              <div className="start-hub-icon-halo">
                <LogoMark size={28} />
              </div>
              <h1 className="start-hub-title">What should we build today?</h1>
              {projectName ? (
                <div className="start-hub-ws-pill">
                  <FolderGit2 size={12} strokeWidth={1.8} />
                  <span>{projectName}</span>
                </div>
              ) : (
                <p className="start-hub-desc">
                  Autonomous coding agent with full codebase awareness, tool execution, and live verification.
                </p>
              )}
            </div>

            {/* Curated 2x2 Starter Action Cards */}
            <div className="start-hub-grid">
              {HERO_STARTERS.map((card, index) => (
                <button
                  key={card.id}
                  type="button"
                  className="start-hub-card"
                  style={{ "--card-index": index } as React.CSSProperties}
                  onClick={() => draftStore.set(card.prompt)}
                >
                  <div className="card-icon-box">
                    <StarterCardIcon name={card.iconName} />
                  </div>
                  <div className="card-text-col">
                    <span className="card-title">{card.title}</span>
                    <span className="card-desc">{card.desc}</span>
                  </div>
                </button>
              ))}
            </div>

            {/* Quick Syntax / Capability Hints */}
            <div className="start-hub-hints">
              <span className="hint-pill">
                <kbd>@</kbd> Reference files
              </span>
              <span className="hint-pill">
                <kbd>/</kbd> Slash commands
              </span>
              <span className="hint-pill">
                <kbd>!</kbd> Bash mode
              </span>
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
