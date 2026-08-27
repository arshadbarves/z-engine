import { useEffect, useRef, useState, useSyncExternalStore } from "react";
import {
  transcriptStore,
  busyStore,
  approvalGateStore,
  toastStore,
  modelStore,
  modeStore,
  draftStore,
  sessionStore,
  setMaxTokens,
  setBusy,
  initEvents,
  submitLocal,
  pushNotice,
  pushToast,
  resolveApproval,
  resetTranscript,
  replaySession,
  resetUsage,
  queueStore,
  type Msg,
} from "./lib/events";
import { configStore } from "./lib/configStore";
import {
  submit,
  compact,
  notes,
  setModel,
  setMode,
  revertLastTurn,
  revertToTurn,
  getConfig,
  approveWithRule,
  deny,
  startSession,
  listSessions,
  deleteSession,
  readSession,
  createWorktree,
} from "./lib/commands";
import { AssistantMarkdown } from "./components/Markdown";
import { ToolCard } from "./components/ToolCard";
import { ApprovalCard } from "./components/ApprovalCard";
import { ThinkingBlock } from "./components/ThinkingBlock";
import { Sidebar } from "./components/Sidebar";
import { Composer } from "./components/Composer";
import { CommandPalette, type PaletteItem } from "./components/CommandPalette";
import { SettingsModal } from "./components/SettingsModal";
import type { SessionEntry } from "./lib/util";
import { workspaceStore, wsBasename } from "./lib/workspaces";
import { refreshCustomCommands } from "./lib/slash";
import { open as openFileDialog } from "@tauri-apps/plugin-dialog";
import { Command, GitCompare, Layers, Plus, RefreshCw, Settings, Undo2 } from "lucide-react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { DiffPanel } from "./components/DiffPanel";
import { WorktreeModal } from "./components/WorktreeModal";

const MODEL_PRESETS = [
  "anthropic/claude-sonnet-4",
  "anthropic/claude-opus-4",
  "openai/gpt-4.1",
  "openai/o4-mini",
  "google/gemini-2.5-pro",
];

const HERO_EXAMPLES = [
  "Fix the failing tests",
  "Explain this codebase in 5 bullets",
  "Add input validation to the CLI",
];

/** In-chat activity row (TUI `working {Ns}s` parity): shimmering while a
 * turn runs; disappears the moment the turn ends or approval is needed.
 * Rendered only while `busy` and keyed per turn so the timer restarts. */
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

/** User bubble with per-message hover actions: copy the text and revert
 * every file change this message's turn (and all later turns) caused.
 * Revert is unavailable for replayed messages — checkpoints are
 * in-memory and do not survive an app restart. */
function UserCard({ m }: { m: Msg }) {
  const busy = useSyncExternalStore(busyStore.subscribe, () => busyStore.getSnapshot());
  const [copied, setCopied] = useState(false);

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
  return (
    <div className="msg user">
      <div className="msg-actions">
        <button title="Copy message" onClick={() => void copy()}>
          {copied ? "copied" : "copy"}
        </button>
        <button
          disabled={!canRevert || busy}
          title={
            canRevert
              ? "Restore files to their state before this message ran"
              : "Checkpoint unavailable (message predates this app launch)"
          }
          onClick={() => canRevert && void revertToTurn(m.runTurn as number)}
        >
          ↺ revert
        </button>
      </div>
      <div className="user-bubble">
        {m.text}
        {m.images && m.images.length > 0 && (
          <span className="msg-images">
            {m.images.map((url, i) => (
              <img key={i} src={url} alt={`attached ${i + 1}`} />
            ))}
          </span>
        )}
      </div>
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
      <ApprovalCard
        m={m}
        onApprove={(d) => onApprove(m, d)}
        onDeny={() => onDeny(m)}
      />
    );
  }
  if (m.kind === "user") {
    return <UserCard m={m} />;
  }
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
  if (m.kind === "tool") {
    return <ToolCard m={m} />;
  }
  if (m.kind === "thinking") {
    return <ThinkingBlock m={m} />;
  }
  if (m.kind === "notice" && m.text.startsWith("$ ")) {
    return <div className="msg notice shell">{m.text}</div>;
  }
  return <div className={`msg ${m.kind}`}>{m.text}</div>;
}

export default function App() {
  const messages = useSyncExternalStore(
    transcriptStore.subscribe,
    () => transcriptStore.getSnapshot(),
  );
  const busy = useSyncExternalStore(busyStore.subscribe, () => busyStore.getSnapshot());
  const toasts = useSyncExternalStore(toastStore.subscribe, () => toastStore.getSnapshot());
  const sessionId = useSyncExternalStore(sessionStore.subscribe, () => sessionStore.getSnapshot());
  const config = useSyncExternalStore(configStore.subscribe, () => configStore.getSnapshot());
  const workspaces = useSyncExternalStore(
    workspaceStore.subscribe,
    () => workspaceStore.getSnapshot(),
  );

  const [sessionsList, setSessionsList] = useState<SessionEntry[]>([]);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [sidebarOpen, setSidebarOpen] = useState(true);
  const [diffOpen, setDiffOpen] = useState(false);
  const [worktreeOpen, setWorktreeOpen] = useState(false);
  const queued = useSyncExternalStore(queueStore.subscribe, () => queueStore.getSnapshot());
  const awaitingApproval = useSyncExternalStore(
    approvalGateStore.subscribe,
    () => approvalGateStore.getSnapshot(),
  );
  const transcriptRef = useRef<HTMLDivElement>(null);

  // Queue flush: when a turn finishes and follow-ups are pending, send the
  // next one (Codex-style continuous queue). An approval-pending gate is
  // separate from `busy` (which only drives the spinner) but also blocks
  // the flush — nothing fires under the approval modal.
  useEffect(() => {
    if (busy || awaitingApproval > 0) return;
    const next = queueStore.shift();
    if (!next) return;
    submitLocal(next.text, next.images);
    setBusy(true);
    void submit(next.text, next.images).catch((e) => {
      // Without this the turn never completes and busy stays true —
      // soft-locking the composer until restart.
      console.error(e);
      setBusy(false);
      pushToast(String(e).replace("Error: ", ""), "warn");
    });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [busy, awaitingApproval]);

  async function createWorktreeAndStart(name: string) {
    try {
      const root = await createWorktree(name);
      await workspaceStore.load();
      workspaceStore.setActive(root);
      setWorktreeOpen(false);
      pushToast(`Worktree created · ${wsBasename(root)}`, "ok");
      await newTask();
    } catch (e) {
      console.error(e);
      pushToast(String(e).replace("Error: ", ""), "warn");
    }
  }

  async function refreshSessions() {
    try {
      const list = (await listSessions()) as unknown as SessionEntry[];
      // Hide brand-new tasks until they carry a first message.
      list.sort((a, b) => Number(b.modifiedMs) - Number(a.modifiedMs));
      setSessionsList(list.filter((s) => s.firstUserMsg != null));
    } catch (e) {
      console.error(e);
    }
  }

  async function openSession(path: string) {
    await startSession(path);
    // Rebuild the transcript from the session JSONL — swapping the agent
    // loop alone leaves the chat area stale.
    try {
      const events = await readSession(path);
      replaySession(events as Parameters<typeof replaySession>[0]);
    } catch (e) {
      console.error("session replay failed:", e);
    }
    resetUsage();
    await refreshSessions();
  }

  async function newTask() {
    await startSession(null, workspaces.active);
    resetTranscript();
    resetUsage();
    void refreshCustomCommands();
    await refreshSessions();
  }

  async function addWorkspace() {
    try {
      const picked = await openFileDialog({
        directory: true,
        multiple: false,
        title: "Choose a workspace folder",
      });
      if (typeof picked === "string" && picked) {
        await workspaceStore.add(picked);
        pushToast(`Workspace added · ${wsBasename(picked)}`, "ok");
      }
    } catch (e) {
      console.error(e);
      pushToast("Could not add workspace", "warn");
    }
  }

  async function removeWorkspace(root: string) {
    try {
      await workspaceStore.remove(root);
      pushToast("Workspace removed", "info");
    } catch (e) {
      console.error(e);
    }
  }

  async function delSession(path: string) {
    // No window.confirm here: it is a silent no-op in Tauri's WKWebView
    // and made the [x] button dead. Delete immediately, say so.
    try {
      await deleteSession(path);
      pushToast("Session deleted", "info");
    } catch (e) {
      console.error(e);
      pushToast("Delete failed", "warn");
    }
    await refreshSessions();
  }

  async function handleApprove(m: Msg, decision: "once" | "session" | "persist") {
    if (m.approvalId == null) return;
    // Trust-boundary logic lives in core (`PolicyEngine::suggested_rule`,
    // delivered on the event); never synthesize rules client-side. If
    // core sent none, degrade to a one-shot approval instead of guessing
    // a wildcard like "bash*" or "rm -rf *".
    let rule = "";
    let effective = decision;
    if (decision !== "once") {
      if (m.suggestedRule) {
        rule = m.suggestedRule;
      } else {
        effective = "once";
      }
    }
    resolveApproval(m.approvalId, decision);
    await approveWithRule(m.approvalId, effective, rule);
  }

  function handleDeny(m: Msg) {
    if (m.approvalId == null) return;
    resolveApproval(m.approvalId, "deny");
    void deny(m.approvalId);
  }

  useEffect(() => {
    void (async () => {
      await initEvents();
      try {
        const cfg = await getConfig();
        configStore.set(cfg);
        if (cfg.model) modelStore.set(cfg.model);
        if (cfg.maxContextTokens) setMaxTokens(Number(cfg.maxContextTokens));
        // TUI-parity startup banner
        pushNotice(
          `harness v${cfg.version ?? "?"} · model ${cfg.model}` +
            (cfg.projectName ? `\nproject ${cfg.projectName}` : ""),
        );
      } catch (e) {
        console.error(e);
      }
      await refreshSessions();
      await workspaceStore.load();
    })();
  }, []);

  useEffect(() => {
    transcriptRef.current?.scrollTo({ top: transcriptRef.current.scrollHeight });
  }, [messages]);

  // macOS double-click-to-zoom parity: with the native title bar hidden,
  // the drag regions (sidebar header zone, chat header) handle dblclick
  // themselves and forward to the window-server zoom action.
  useEffect(() => {
    function onDblClick(e: MouseEvent) {
      const target = e.target as HTMLElement;
      if (target.closest("button, input, textarea, a, .session, .ws-head")) return;
      if (target.closest(".sidebar, .chat-head")) {
        void getCurrentWindow().toggleMaximize();
      }
    }
    window.addEventListener("dblclick", onDblClick);
    return () => window.removeEventListener("dblclick", onDblClick);
  }, []);

  // global shortcuts: ⌘K palette · ⌘N new task · ⌘B toggle sidebar
  const newTaskRef = useRef(newTask);  useEffect(() => {
    newTaskRef.current = newTask;
  });
  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      if (e.metaKey || e.ctrlKey) {
        const k = e.key.toLowerCase();
        if (k === "k") {
          e.preventDefault();
          setPaletteOpen((o) => !o);
        } else if (k === "n") {
          e.preventDefault();
          void newTaskRef.current();
        } else if (k === "b") {
          e.preventDefault();
          setSidebarOpen((o) => !o);
        }
      }
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  function paletteActions(): PaletteItem[] {
    const modeOrder = ["normal", "accept-edits", "plan"];
    const nextMode =
      modeOrder[(modeOrder.indexOf(modeStore.getSnapshot()) + 1) % modeOrder.length];
    return [
      { label: "New task", hint: "session", keywords: "new task session", run: () => void newTask() },
      {
        label: "Add workspace…",
        hint: "project",
        keywords: "add open folder workspace project",
        run: () => void addWorkspace(),
      },
      {
        label: "New task in git worktree…",
        hint: "isolated branch",
        keywords: "worktree branch isolate parallel task new",
        run: () => setWorktreeOpen(true),
      },
      {
        label: "Review changes",
        hint: "diff panel",
        keywords: "diff review changes files git",
        run: () => setDiffOpen(true),
      },
      { label: "/compact — force context compaction", hint: "context", keywords: "compact context", run: () => void compact() },
      { label: "/notes — dump durable context notes", hint: "notes", keywords: "notes context", run: () => void notes() },
      { label: "Rewind last turn", hint: "files", keywords: "revert undo rewind", run: () => void revertLastTurn() },
      { label: "Open settings…", hint: "app", keywords: "settings preferences config permissions mcp cost", run: () => setSettingsOpen(true) },
      {
        label: `Set permission mode · ${nextMode}`,
        hint: "mode",
        keywords: "mode permission auto accept plan normal",
        run: () => {
          modeStore.set(nextMode);
          void setMode(nextMode);
        },
      },
      ...MODEL_PRESETS.map((p) => ({
        label: `Switch model · ${p}`,
        hint: "model",
        keywords: `model switch ${p}`,
        run: () => {
          void setModel(p).then(() => modelStore.set(p));
        },
      })),
      {
        label: "Toggle sidebar",
        hint: "⌘B",
        keywords: "toggle sidebar view",
        run: () => setSidebarOpen((o) => !o),
      },
      ...HERO_EXAMPLES.map((ex) => ({
        label: ex,
        hint: "start a task",
        keywords: "task example prompt",
        run: () => {
          draftStore.set("");
          submitLocal(ex);
          void submit(ex).catch(console.error);
        },
      })),
    ];
  }

  return (
    <main className={`app${sidebarOpen ? "" : " no-sidebar"}`}>
      <aside className="sidebar">
        <div className="brand">
          <Settings size={12} />
          <span>harness</span>
        </div>
        <button className="newtask" onClick={() => void newTask()}>
          <Plus size={13} />
          New task
        </button>
        <div className="sess-head">
          <span>Sessions</span>
          <button className="mini" title="Refresh" onClick={() => void refreshSessions()}>
            <RefreshCw size={12} />
          </button>
        </div>
        <Sidebar
          sessions={sessionsList}
          workspaces={workspaces.roots}
          activeWorkspace={workspaces.active}
          onOpen={(p) => void openSession(p)}
          onDelete={(p) => void delSession(p)}
          onAddWorkspace={() => void addWorkspace()}
          onRemoveWorkspace={(root) => void removeWorkspace(root)}
          onActivateWorkspace={(root) => workspaceStore.setActive(root)}
        />
        <button className="side-foot gear" title="Settings" onClick={() => setSettingsOpen(true)}>
          <Settings size={13} />
          Settings
          <span className="side-foot-note">
            {config?.version ? `v${config.version}` : "local-first"}
          </span>
        </button>
      </aside>

      <section className="chat">
        <header className="chat-head">
          <div
            className="chat-title"
            title={
              workspaces.active
                ? `workspace ${workspaces.active}${sessionId ? ` · session ${sessionId}` : ""}`
                : sessionId
                  ? `session ${sessionId}`
                  : undefined
            }
          >
            {workspaces.active ? wsBasename(workspaces.active) : config?.projectName || "harness"}
          </div>
          <div className="head-controls">
            <button
              className="icon-btn"
              title="Command palette (⌘K)"
              onClick={() => setPaletteOpen(true)}
            >
              <Command size={12} />
            </button>
            <button
              className="icon-btn"
              title="/compact — force context compaction"
              disabled={busy}
              onClick={() => void compact()}
            >
              <Layers size={12} />
            </button>
            <button
              className="icon-btn"
              title="Rewind — undo the last turn's file changes"
              disabled={busy}
              onClick={() => void revertLastTurn()}
            >
              <Undo2 size={12} />
            </button>
            <button
              className={`icon-btn${diffOpen ? " active" : ""}`}
              title="Review changes (working tree vs HEAD)"
              onClick={() => setDiffOpen((o) => !o)}
            >
              <GitCompare size={12} />
            </button>
          </div>
        </header>

        <div className="transcript" ref={transcriptRef}>
          <div className="transcript-inner">
            {messages.length === 0 && (
              <div className="hero">
                <Settings size={12} />
                <h1>What are we building?</h1>
                <p>
                  Describe a task — harness reads files, runs commands, edits code
                  and verifies the result before reporting back.
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
            {messages.map((m: Msg) => (
              <MsgCard
                key={m.id}
                m={m}
                onApprove={(m, d) => void handleApprove(m, d)}
                onDeny={(m) => handleDeny(m)}
              />
            ))}
            {busy && <WorkingRow key={`working-${messages.length}`} />}
          </div>
        </div>

        {diffOpen && <DiffPanel onClose={() => setDiffOpen(false)} />}
        {queued.length > 0 && (
          <div className="queue-strip">
            <span className="queue-label">queued</span>
            {queued.map((q, i) => (
              <span key={i} className="queue-pill" title={q.text}>
                {q.text.slice(0, 48) || `(${q.images.length} image(s))`}
                <button title="Remove from queue" onClick={() => queueStore.removeAt(i)}>
                  ×
                </button>
              </span>
            ))}
          </div>
        )}

        <Composer />
      </section>

      {paletteOpen && (
        <CommandPalette
          onClose={() => setPaletteOpen(false)}
          sessions={sessionsList}
          workspaces={workspaces.roots}
          activeWorkspace={workspaces.active}
          actions={paletteActions()}
          onOpenSession={(p) => void openSession(p)}
          onActivateWorkspace={(root) => workspaceStore.setActive(root)}
        />
      )}
      {settingsOpen && <SettingsModal onClose={() => setSettingsOpen(false)} />}
      {worktreeOpen && (
        <WorktreeModal onClose={() => setWorktreeOpen(false)} onCreate={(n) => void createWorktreeAndStart(n)} />
      )}

      <div className="toasts" aria-live="polite">
        {toasts.map((t) => (
          <div key={t.id} className={`toast ${t.tone}`}>
            {t.text}
          </div>
        ))}
      </div>
    </main>
  );
}
