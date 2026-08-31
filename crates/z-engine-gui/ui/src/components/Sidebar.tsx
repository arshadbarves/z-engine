import { useEffect, useMemo, useRef, useState } from "react";
import {
  ChevronDown,
  ChevronRight,
  FolderGit2,
  LoaderCircle,
  MessageSquare,
  Plus,
  Search,
  ShieldAlert,
  Trash2,
  X,
} from "../lib/icons";
import type { SessionActivity } from "../lib/events";
import { filterSessions, type SessionEntry } from "../lib/util";
import { sessionLabel } from "../lib/sessionList";
import { wsBasename, sameWorkspacePath } from "../lib/workspaces";

/* ── Session Row Component ─────────────────────────────────────────────── */

function SessionTreeItem({
  session,
  active,
  activityState,
  onOpen,
  onDelete,
}: {
  session: SessionEntry;
  active: boolean;
  activityState: SessionActivity | null;
  onOpen: (path: string, projectRoot?: string | null) => void;
  onDelete: (path: string) => void;
}) {
  const title = sessionLabel(session.firstUserMsg);
  const isWorking = activityState === "working";
  const isApproval = activityState === "approval";
  const unreadOutcome =
    !active && !activityState && (session.unreadOutcome === "completed" || session.unreadOutcome === "aborted")
      ? session.unreadOutcome
      : null;

  return (
    <div
      className={`sidebar-session-item${active ? " active" : ""}${
        isWorking ? " working" : ""
      }${isApproval ? " approval" : ""}${
        unreadOutcome ? ` unread unread-${unreadOutcome}` : ""
      }`}
      role="button"
      tabIndex={0}
      title={
        isApproval
          ? `Action Required · ${title}`
          : isWorking
            ? `Agent Working · ${title}`
            : title
      }
      onClick={(e) => {
        e.stopPropagation();
        onOpen(session.path, session.projectRoot);
      }}
      onKeyDown={(e) => e.key === "Enter" && onOpen(session.path, session.projectRoot)}
    >
      <div className="session-item-icon-wrap">
        {isWorking ? (
          <LoaderCircle size={13} className="spin session-spin-icon" strokeWidth={2} />
        ) : isApproval ? (
          <ShieldAlert size={13} className="session-alert-icon" strokeWidth={2} />
        ) : (
          <MessageSquare size={13} className="session-msg-icon" strokeWidth={1.8} />
        )}
      </div>

      <span className="session-item-title">{title}</span>

      <div className="session-item-tail">
        {unreadOutcome && (
          <span
            className={`session-status-dot dot-${unreadOutcome}`}
            title={unreadOutcome === "completed" ? "Completed" : "Stopped"}
            aria-label={unreadOutcome}
          />
        )}
        <button
          type="button"
          className="session-delete-btn"
          title="Delete chat"
          onClick={(e) => {
            e.stopPropagation();
            onDelete(session.path);
          }}
        >
          <Trash2 size={11} strokeWidth={1.8} />
        </button>
      </div>
    </div>
  );
}

/* ── Workspace Accordion Item ──────────────────────────────────────────── */

function WorkspaceTreeItem({
  root,
  active,
  sessions,
  activeUlid,
  activity,
  onOpen,
  onDelete,
  onActivate,
  onRemove,
}: {
  root: string;
  active: boolean;
  sessions: SessionEntry[];
  activeUlid: string;
  activity: Record<string, SessionActivity>;
  onOpen: (path: string, projectRoot?: string | null) => void;
  onDelete: (path: string) => void;
  onActivate: (root: string) => void;
  onRemove: (root: string) => void;
}) {
  const [open, setOpen] = useState(active);
  const wasActive = useRef(active);

  useEffect(() => {
    if (active && !wasActive.current) setOpen(true);
    wasActive.current = active;
  }, [active]);

  const items = useMemo(() => sessions.slice(0, 40), [sessions]);
  const name = wsBasename(root);

  // Derive any live activity in this workspace
  const workspaceActivity = useMemo<SessionActivity | null>(() => {
    let working = false;
    for (const s of items) {
      const a = activity[s.ulid];
      if (a === "approval") return "approval";
      if (a === "working") working = true;
    }
    return working ? "working" : null;
  }, [items, activity]);

  return (
    <div className={`workspace-item${active ? " active-ws" : ""}`}>
      <div
        className={`workspace-header${workspaceActivity ? ` ws-${workspaceActivity}` : ""}`}
        role="button"
        tabIndex={0}
        title={`${root}${active ? " (Active Workspace)" : ""}`}
        onClick={() => {
          onActivate(root);
          setOpen((o) => !o);
        }}
        onKeyDown={(e) => e.key === "Enter" && onActivate(root)}
      >
        <span className="workspace-chevron" aria-hidden="true">
          {open ? <ChevronDown size={11} strokeWidth={2} /> : <ChevronRight size={11} strokeWidth={2} />}
        </span>

        <FolderGit2 size={13} className="workspace-folder-icon" strokeWidth={1.8} />
        <span className="workspace-title">{name}</span>

        <div className="workspace-actions">
          {items.length > 0 && <span className="workspace-badge">{items.length}</span>}
          <button
            type="button"
            className="workspace-del-btn"
            title="Remove workspace"
            onClick={(e) => {
              e.stopPropagation();
              onRemove(root);
            }}
          >
            <Trash2 size={11} strokeWidth={1.8} />
          </button>
        </div>
      </div>

      {open && (
        <div className="workspace-session-list">
          {items.length === 0 ? (
            <div className="workspace-empty-hint">No chats in this workspace</div>
          ) : (
            items.map((s) => (
              <SessionTreeItem
                key={s.path}
                session={s}
                active={s.ulid === activeUlid}
                activityState={activity[s.ulid] ?? null}
                onOpen={onOpen}
                onDelete={onDelete}
              />
            ))
          )}
        </div>
      )}
    </div>
  );
}

/* ── Main Sidebar View ─────────────────────────────────────────────────── */

export function Sidebar({
  sessions,
  workspaces,
  activeWorkspace,
  activeUlid,
  activity,
  onOpen,
  onDelete,
  onAddWorkspace,
  onRemoveWorkspace,
  onActivateWorkspace,
}: {
  sessions: SessionEntry[];
  workspaces: string[];
  activeWorkspace: string | null;
  activeUlid: string;
  activity: Record<string, SessionActivity>;
  onOpen: (path: string, projectRoot?: string | null) => void;
  onDelete: (path: string) => void;
  onAddWorkspace: () => void;
  onRemoveWorkspace: (root: string) => void;
  onActivateWorkspace: (root: string | null) => void;
}) {
  const [query, setQuery] = useState("");
  const [recentsOpen, setRecentsOpen] = useState(true);

  const filtered = useMemo(() => filterSessions(sessions, query), [sessions, query]);

  // Split sessions by workspace and loose/recents
  const { byWorkspace, otherSessions } = useMemo(() => {
    const byWorkspace = new Map<string, SessionEntry[]>();
    for (const root of workspaces) byWorkspace.set(root, []);
    const otherSessions: SessionEntry[] = [];

    for (const s of filtered) {
      const hit = s.projectRoot
        ? workspaces.find((root) => sameWorkspacePath(s.projectRoot, root))
        : undefined;
      if (hit) byWorkspace.get(hit)!.push(s);
      else otherSessions.push(s);
    }
    return { byWorkspace, otherSessions };
  }, [filtered, workspaces]);

  return (
    <div className="sidebar-content-deck">
      {/* Search Input */}
      <div className="sidebar-search-box">
        <Search size={12} className="sidebar-search-icon" strokeWidth={1.8} />
        <input
          type="text"
          value={query}
          onChange={(e) => setQuery(e.currentTarget.value)}
          placeholder="Search chats…"
          spellCheck={false}
          className="sidebar-search-input"
        />
        {query && (
          <button
            type="button"
            className="sidebar-search-clear"
            title="Clear search"
            onClick={() => setQuery("")}
          >
            <X size={11} strokeWidth={2} />
          </button>
        )}
      </div>

      {/* Projects & Workspaces Section */}
      <div className="sidebar-scrollable-area">
        <div className="sidebar-group-header">
          <span className="group-title">Workspaces</span>
          <button
            type="button"
            className="group-action-btn"
            title="Add workspace folder…"
            onClick={onAddWorkspace}
          >
            <Plus size={12} strokeWidth={2} />
          </button>
        </div>

        {workspaces.length === 0 && (
          <div className="sidebar-empty-state">
            <span>No workspaces linked.</span>
            <button type="button" className="empty-add-btn" onClick={onAddWorkspace}>
              Add folder
            </button>
          </div>
        )}

        {workspaces.map((root) => (
          <WorkspaceTreeItem
            key={root}
            root={root}
            active={sameWorkspacePath(activeWorkspace, root)}
            sessions={byWorkspace.get(root) ?? []}
            activeUlid={activeUlid}
            activity={activity}
            onOpen={onOpen}
            onDelete={onDelete}
            onActivate={onActivateWorkspace}
            onRemove={onRemoveWorkspace}
          />
        ))}

        {/* Loose / Recents Chats */}
        {otherSessions.length > 0 && (
          <div className="sidebar-group-section">
            <div
              className="sidebar-group-header clickable"
              role="button"
              tabIndex={0}
              onClick={() => setRecentsOpen((o) => !o)}
              onKeyDown={(e) => e.key === "Enter" && setRecentsOpen((o) => !o)}
            >
              <span className="group-title">Other Chats</span>
              <span className="workspace-badge">{otherSessions.length}</span>
            </div>

            {recentsOpen && (
              <div className="loose-sessions-list">
                {otherSessions.slice(0, 24).map((s) => (
                  <SessionTreeItem
                    key={s.path}
                    session={s}
                    active={s.ulid === activeUlid}
                    activityState={activity[s.ulid] ?? null}
                    onOpen={onOpen}
                    onDelete={onDelete}
                  />
                ))}
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  );
}

