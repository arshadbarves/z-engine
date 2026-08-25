import { useEffect, useRef, useState, useSyncExternalStore } from "react";
import {
  transcriptStore,
  busyStore,
  setBusy,
  initEvents,
  submitLocal,
  type Msg,
} from "./lib/events";
import {
  submit,
  abort,
  setMode,
  approveWithRule,
  deny,
  startSession,
  listSessions,
  deleteSession,
} from "./lib/commands";

interface SessionEntry {
  path: string;
  ulid: string;
  firstUserMsg: string | null;
  modifiedMs: number;
}

const windowEvents: Array<() => void> = [];
let sessions: SessionEntry[] = [];

function useSessions(): SessionEntry[] {
  return useSyncExternalStore(
    (l) => {
      windowEvents.push(l);
      return () => {
        const i = windowEvents.indexOf(l);
        if (i >= 0) windowEvents.splice(i, 1);
      };
    },
    () => sessions,
  );
}

async function refreshSessions() {
  try {
    const list = (await listSessions()) as unknown as SessionEntry[];
    list.sort((a, b) => Number(b.modifiedMs) - Number(a.modifiedMs));
    sessions = list;
  } catch (e) {
    console.error(e);
  }
}

async function openSession(path: string) {
  await startSession(path);
  await refreshSessions();
}

async function newTask() {
  await startSession(null);
  await refreshSessions();
}

async function delSession(path: string) {
  if (!confirm("Delete this session transcript?")) return;
  await deleteSession(path);
  await refreshSessions();
}

function relTime(ms: number): string {
  const d = Date.now() - ms;
  if (d < 60_000) return "now";
  if (d < 3_600_000) return `${Math.floor(d / 60_000)}m`;
  if (d < 86_400_000) return `${Math.floor(d / 3_600_000)}h`;
  return `${Math.floor(d / 86_400_000)}d`;
}

function MsgCard({ m }: { m: Msg }) {
  const cls = `msg ${m.kind}${m.streaming ? " streaming" : ""}`;
  if (m.kind === "approval") {
    const rule =
      m.bashCommand
        ? m.bashCommand.split(/\s+/).slice(0, 2).join(" ") + "*"
        : m.suggestedRule ?? "bash*";
    return (
      <div className={cls}>
        <div className="approval-body">{m.text}</div>
        <div className="approval-actions">
          <button
            className="ok"
            onClick={() => void approveWithRule(m.approvalId!, "session", rule)}
          >
            2 · Always (session)
          </button>
          {m.canPersist && (
            <button
              className="ok"
              onClick={() =>
                void approveWithRule(m.approvalId!, "persist", m.suggestedRule ?? "bash*")
              }
            >
              3 · Persist
            </button>
          )}
          <button className="deny" onClick={() => void deny(m.approvalId!)}>
            4 · Deny
          </button>
          <span className="hint">1/y = once only</span>
        </div>
      </div>
    );
  }
  if (m.kind === "assistant") {
    return (
      <div
        className={cls}
        dangerouslySetInnerHTML={{
          __html: m.text
            .replace(/&/g, "&amp;")
            .replace(/</g, "&lt;")
            .replace(/\n/g, "<br/>"),
        }}
      />
    );
  }
  return <div className={cls}>{m.text}</div>;
}

export default function App() {
  const messages = useSyncExternalStore(
    transcriptStore.subscribe,
    () => transcriptStore.getSnapshot(),
  );
  const busy = useSyncExternalStore(busyStore.subscribe, () => busyStore.getSnapshot());
  const sessions = useSessions();

  const [input, setInput] = useState("");
  const [mode, setModeState] = useState("normal");
  const transcriptRef = useRef<HTMLDivElement>(null);

  async function send() {
    const text = input.trim();
    if (!text || busy) return;
    setInput("");
    submitLocal(text);
    setBusy(true);
    try {
      await submit(text);
    } catch (e) {
      console.error(e);
    }
  }

  useEffect(() => {
    void (async () => {
      await initEvents();
      await refreshSessions();
    })();
  }, []);

  useEffect(() => {
    transcriptRef.current?.scrollTo({ top: transcriptRef.current.scrollHeight });
  }, [messages]);

  return (
    <main className="app">
      <aside className="sidebar">
        <div className="brand">harness</div>
        <button className="newtask" onClick={() => void newTask()}>
          ＋ New task
        </button>
        <div className="sess-head">
          <span>sessions</span>
          <button className="mini" title="refresh" onClick={() => void refreshSessions()}>
            ↻
          </button>
        </div>
        <div className="sessions">
          {sessions.length === 0 ? (
            <div className="sess-empty">no sessions yet</div>
          ) : (
            sessions.map((s) => (
              <div
                key={s.path}
                className="session"
                role="button"
                tabIndex={0}
                onClick={() => void openSession(s.path)}
                onKeyDown={(e) => e.key === "Enter" && void openSession(s.path)}
              >
                <div className="sess-preview">{s.firstUserMsg ?? "(empty)"}</div>
                <div className="sess-meta">
                  <span>{s.ulid.slice(0, 6)}</span>
                  <span>{relTime(Number(s.modifiedMs))}</span>
                  <button
                    className="del"
                    title="delete"
                    onClick={(e) => {
                      e.stopPropagation();
                      void delSession(s.path);
                    }}
                  >
                    ✕
                  </button>
                </div>
              </div>
            ))
          )}
        </div>
        <div className="spacer" />
        <div className="side-note">
          TUI remains available for
          <br />
          keyboard-only use
        </div>
      </aside>

      <section className="chat">
        <div className="transcript" ref={transcriptRef}>
          {messages.map((m: Msg) => (
            <MsgCard key={m.id} m={m} />
          ))}
        </div>

        <div className="composer">
          <select
            className="mode"
            title="permission mode"
            value={mode}
            onChange={(e) => {
              const v = e.currentTarget.value;
              setModeState(v);
              void setMode(v);
            }}
          >
            <option value="normal">normal</option>
            <option value="accept-edits">auto-accept edits</option>
            <option value="plan">plan</option>
          </select>
          <textarea
            rows={2}
            placeholder={
              busy ? "working… (Stop to abort)" : "type a task — Shift+Enter for newline"
            }
            value={input}
            onChange={(e) => setInput(e.currentTarget.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && !e.shiftKey) {
                e.preventDefault();
                void send();
              }
            }}
          ></textarea>
          {busy ? (
            <button className="stop" onClick={() => void abort()}>
              ■ Stop
            </button>
          ) : (
            <button className="send" onClick={() => void send()} disabled={!input.trim()}>
              Send ⏎
            </button>
          )}
        </div>
      </section>
    </main>
  );
}
