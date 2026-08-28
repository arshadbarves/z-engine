import { useEffect, useMemo, useRef, useState } from "react";
import { ChevronRight, Folder, Plus, Search, Trash2 } from "lucide-react";
import type { SessionActivity } from "../lib/events";
import { filterSessions, groupSessions, type SessionEntry } from "../lib/util";
import { wsBasename } from "../lib/workspaces";
import { SessionRow } from "./SessionRow";

function WorkspaceRow({
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
  const groups = useMemo(() => groupSessions(items), [items]);

  // Approval outranks working: a blocked session needs the user's eye first.
  const projectActivity = useMemo<SessionActivity | null>(() => {
    let working = false;
    for (const s of items) {
      const a = activity[s.ulid];
      if (a === "approval") return "approval";
      if (a === "working") working = true;
    }
    return working ? "working" : null;
  }, [items, activity]);

  return (
    <div className={`ws-row${active ? " active" : ""}`}>
      <div
        className={`ws-head${projectActivity ? ` ${projectActivity}` : ""}`}
        role="button"
        tabIndex={0}
        title={`${root}${active ? " · active" : " · click to select"}`}
        onClick={() => {
          onActivate(root);
          setOpen((o) => !o);
        }}
        onKeyDown={(e) => e.key === "Enter" && onActivate(root)}
      >
        <span className={`ws-chevron${open ? " open" : ""}`}>
          <ChevronRight size={11} />
        </span>
        <Folder size={13} className="ws-icon" />
        <span className="ws-name">{wsBasename(root)}</span>
        <span className="ws-tail">
          <span className="ws-count">{items.length || ""}</span>
          <button
            className="del"
            title="Remove workspace from list (sessions are kept)"
            onClick={(e) => {
              e.stopPropagation();
              onRemove(root);
            }}
          >
            <Trash2 size={11} />
          </button>
        </span>
      </div>
      {open && (
        <div className="ws-sessions">
          {items.length === 0 ? (
            <div className="sess-empty">No conversations yet</div>
          ) : (
            groups.map((g) => (
              <div key={g.label} className="sess-group">
                <div className="sess-group-label">{g.label}</div>
                {g.items.map((s) => (
                  <SessionRow
                    key={s.path}
                    s={s}
                    active={s.ulid === activeUlid}
                    state={activity[s.ulid] ?? null}
                    onOpen={onOpen}
                    onDelete={onDelete}
                  />
                ))}
              </div>
            ))
          )}
        </div>
      )}
    </div>
  );
}

/** Human-centric sessions sidebar: search, workspaces, chronological day groupings. */
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

  const byWs = useMemo(() => {
    const m = new Map<string, SessionEntry[]>();
    for (const root of workspaces) m.set(root, []);
    const other: SessionEntry[] = [];
    for (const s of filtered) {
      const hit = s.projectRoot ? m.get(s.projectRoot) : undefined;
      if (hit) hit.push(s);
      else other.push(s);
    }
    return { m, other };
  }, [filtered, workspaces]);

  const otherGroups = useMemo(
    () => groupSessions(byWs.other.slice(0, 30)),
    [byWs.other],
  );

  return (
    <div className="sessions">
      <div className="sess-search">
        <Search size={13} className="sess-search-icon" />
        <input
          value={query}
          onChange={(e) => setQuery(e.currentTarget.value)}
          placeholder="Search conversations…"
          spellCheck={false}
        />
      </div>

      <div className="sess-list">
        <div className="ws-section-head">
          <span>Workspaces</span>
          <button className="mini" title="Add workspace folder…" onClick={onAddWorkspace}>
            <Plus size={12} />
          </button>
        </div>

        {workspaces.length === 0 && (
          <div className="sess-empty">No workspace added yet.</div>
        )}

        {workspaces.map((root) => (
          <WorkspaceRow
            key={root}
            root={root}
            active={activeWorkspace === root}
            sessions={byWs.m.get(root) ?? []}
            activeUlid={activeUlid}
            activity={activity}
            onOpen={onOpen}
            onDelete={onDelete}
            onActivate={onActivateWorkspace}
            onRemove={onRemoveWorkspace}
          />
        ))}

        {byWs.other.length > 0 && (
          <>
            <div
              className="ws-section-head other"
              role="button"
              tabIndex={0}
              onClick={() => setRecentsOpen((o) => !o)}
              onKeyDown={(e) => e.key === "Enter" && setRecentsOpen((o) => !o)}
            >
              <div className="ws-head-left">
                <ChevronRight
                  size={11}
                  className={`ws-chevron${recentsOpen ? " open" : ""}`}
                />
                <span>Recent Conversations</span>
              </div>
              <span className="ws-count">{byWs.other.length}</span>
            </div>

            {recentsOpen &&
              otherGroups.map((g) => (
                <div key={g.label} className="sess-group">
                  <div className="sess-group-label">{g.label}</div>
                  {g.items.map((s) => (
                    <SessionRow
                      key={s.path}
                      s={s}
                      active={s.ulid === activeUlid}
                      state={activity[s.ulid] ?? null}
                      onOpen={onOpen}
                      onDelete={onDelete}
                    />
                  ))}
                </div>
              ))}
          </>
        )}
      </div>
    </div>
  );
}
