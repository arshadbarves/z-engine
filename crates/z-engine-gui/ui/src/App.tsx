import { useEffect, useRef, useState, useSyncExternalStore } from "react";
import {
  transcriptStore,
  busyStore,
  approvalGateStore,
  toastStore,
  sessionStore,
  modelStore,
  setMaxTokens,
  initEvents,
  pushToast,
  resolveApproval,
  queueStore,
  drainReadyQueues,
  submitOnSession,
  sessionActivityStore,
  sessionsTickStore,
  hydrateStore,
  type Msg,
} from "./lib/events";
import { configStore } from "./lib/configStore";
import {
  submit,
  getConfig,
  approveWithRule,
  deny,
  listSessions,
  deleteSession,
  createWorktree,
} from "./lib/commands";
import { hydrateNewSession, hydrateOpenSession } from "./lib/sessionOpen";
import { applyFirstUserTitle, mergeSessionLists, newSessionEntry, upsertSession } from "./lib/sessionList";
import { Composer } from "./components/Composer";
import { CommandPalette } from "./components/CommandPalette";
import { SettingsPage } from "./components/settings/SettingsPage";
import { SplashScreen } from "./components/SplashScreen";
import { ChatHeader, JumpLatest } from "./components/ChatHeader";
import { MsgList } from "./components/MsgList";
import { AppSidebar } from "./components/AppSidebar";
import { paletteActions } from "./lib/paletteActions";
import type { SessionEntry } from "./lib/util";
import { workspaceStore, wsBasename } from "./lib/workspaces";
import { refreshCustomCommands } from "./lib/slash";
import { open as openFileDialog } from "@tauri-apps/plugin-dialog";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { DiffPanel } from "./components/DiffPanel";
import { WorktreeModal } from "./components/WorktreeModal";

export default function App() {
  const messages = useSyncExternalStore(
    transcriptStore.subscribe,
    () => transcriptStore.getSnapshot(),
  );
  const busy = useSyncExternalStore(busyStore.subscribe, () => busyStore.getSnapshot());
  const toasts = useSyncExternalStore(toastStore.subscribe, () => toastStore.getSnapshot());
  const sessionId = useSyncExternalStore(sessionStore.subscribe, () => sessionStore.getSnapshot());
  const sessionsTick = useSyncExternalStore(
    sessionsTickStore.subscribe,
    () => sessionsTickStore.getSnapshot(),
  );
  const config = useSyncExternalStore(configStore.subscribe, () => configStore.getSnapshot());
  const workspaces = useSyncExternalStore(
    workspaceStore.subscribe,
    () => workspaceStore.getSnapshot(),
  );

  const [sessionsList, setSessionsList] = useState<SessionEntry[]>([]);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [splash, setSplash] = useState(true);
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [sidebarOpen, setSidebarOpen] = useState(true);
  const [diffOpen, setDiffOpen] = useState(false);
  const [worktreeOpen, setWorktreeOpen] = useState(false);
  const queued = useSyncExternalStore(queueStore.subscribe, () => queueStore.getSnapshot());
  const awaitingApproval = useSyncExternalStore(
    approvalGateStore.subscribe,
    () => approvalGateStore.getSnapshot(),
  );
  const sessionActivity = useSyncExternalStore(
    sessionActivityStore.subscribe,
    () => sessionActivityStore.getSnapshot(),
  );
  const transcriptRef = useRef<HTMLDivElement>(null);
  const stickToBottom = useRef(true);
  const [showJump, setShowJump] = useState(false);
  const hydrating = useSyncExternalStore(
    hydrateStore.subscribe,
    () => hydrateStore.getSnapshot(),
  );

  // Queue flush: idle sessions (foreground or background) send the next
  // follow-up. Approval-pending still blocks that session only.
  useEffect(() => {
    const jobs = drainReadyQueues();
    for (const job of jobs) {
      submitOnSession(job.sessionId, job.text, job.images);
      void submit(job.text, job.images, job.sessionId).catch((e) => {
        console.error(e);
        pushToast(String(e).replace("Error: ", ""), "warn");
      });
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [busy, awaitingApproval, sessionActivity]);

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
      setSessionsList((prev) => mergeSessionLists(list, prev));
    } catch (e) {
      console.error(e);
    }
  }

  async function openSession(path: string, projectRoot?: string | null) {
    stickToBottom.current = true;
    setShowJump(false);
    if (projectRoot) workspaceStore.setActive(projectRoot);
    await hydrateOpenSession(path, projectRoot);
    await refreshSessions();
  }

  async function newTask() {
    const created = await hydrateNewSession(workspaces.active);
    if (created?.path) {
      setSessionsList((prev) =>
        upsertSession(prev, newSessionEntry(created.ulid, created.path, workspaces.active)),
      );
    }
    void refreshCustomCommands();
    void refreshSessions();
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
      } catch (e) {
        console.error(e);
      }
      await refreshSessions();
      await workspaceStore.load();
    })();
  }, []);

  useEffect(() => {
    if (sessionsTick === 0) return;
    void refreshSessions();
  }, [sessionsTick]);

  useEffect(() => {
    const last = messages[messages.length - 1];
    if (last?.kind === "user") {
      stickToBottom.current = true;
      setShowJump(false);
      setSessionsList((prev) => applyFirstUserTitle(prev, sessionId, messages));
    }
    if (!stickToBottom.current) return;
    transcriptRef.current?.scrollTo({ top: transcriptRef.current.scrollHeight });
  }, [messages, sessionId]);

  function onTranscriptScroll() {
    const el = transcriptRef.current;
    if (!el) return;
    const gap = el.scrollHeight - el.scrollTop - el.clientHeight;
    const atBottom = gap < 72;
    stickToBottom.current = atBottom;
    setShowJump(!atBottom);
  }

  function jumpToLatest() {
    stickToBottom.current = true;
    setShowJump(false);
    transcriptRef.current?.scrollTo({ top: transcriptRef.current.scrollHeight, behavior: "smooth" });
  }

  // Overlay title bar: dblclick sidebar/header zooms like a native window.
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

  return (
    <>
      {splash && <SplashScreen onDone={() => setSplash(false)} />}
    <main className={`app${sidebarOpen ? "" : " no-sidebar"}`}>
      <AppSidebar
        sessions={sessionsList}
        workspaces={workspaces.roots}
        activeWorkspace={workspaces.active}
        activeUlid={sessionId}
        activity={sessionActivity}
        version={config?.version}
        onNewChat={() => void newTask()}
        onOpen={(p, root) => void openSession(p, root)}
        onDelete={(p) => void delSession(p)}
        onAddWorkspace={() => void addWorkspace()}
        onRemoveWorkspace={(root) => void removeWorkspace(root)}
        onActivateWorkspace={(root) => workspaceStore.setActive(root)}
        onSettings={() => setSettingsOpen(true)}
      />

      <section className="chat">
        <ChatHeader
          title={workspaces.active ? wsBasename(workspaces.active) : config?.projectName || "Z Engine"}
          titleHint={
            workspaces.active
              ? `workspace ${workspaces.active}${sessionId ? ` · session ${sessionId}` : ""}`
              : sessionId
                ? `session ${sessionId}`
                : undefined
          }
          diffOpen={diffOpen}
          sidebarOpen={sidebarOpen}
          onToggleSidebar={() => setSidebarOpen((o) => !o)}
          onPalette={() => setPaletteOpen(true)}
          onToggleDiff={() => setDiffOpen((o) => !o)}
        />

        <div className="transcript-wrap">
          {hydrating && <div className="hydrate-shimmer" aria-label="Restoring chat" />}
          <div className="transcript" ref={transcriptRef} onScroll={onTranscriptScroll}>
            <MsgList
              messages={messages}
              busy={busy}
              projectName={workspaces.active ? wsBasename(workspaces.active) : null}
              onApprove={(m, d) => void handleApprove(m, d)}
              onDeny={(m) => handleDeny(m)}
            />
          </div>
          {showJump && <JumpLatest onJump={jumpToLatest} />}
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
          actions={paletteActions({
            newTask: () => void newTask(),
            addWorkspace: () => void addWorkspace(),
            openWorktree: () => setWorktreeOpen(true),
            openDiff: () => setDiffOpen(true),
            openSettings: () => setSettingsOpen(true),
            toggleSidebar: () => setSidebarOpen((o) => !o),
          })}
          onOpenSession={(p, root) => void openSession(p, root)}
          onActivateWorkspace={(root) => workspaceStore.setActive(root)}
        />
      )}
      {settingsOpen && <SettingsPage onClose={() => setSettingsOpen(false)} />}
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
    </>
  );
}
