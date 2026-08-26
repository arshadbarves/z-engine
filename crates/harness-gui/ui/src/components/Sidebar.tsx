import { useEffect, useMemo, useRef, useState } from "react";
import { ChevronRight, Folder, Plus, Search, Trash2 } from "lucide-react";
import {
  filterSessions,
  relTime,
  type SessionEntry,
} from "../lib/util";
import { wsBasename } from "../lib/workspaces";

/** One workspace row: folder glyph + name; expands to its sessions. */
function WorkspaceRow({
  root,
  active,
  sessions,
  query,
  onOpen,
  onDelete,
  onActivate,
  onRemove,
}: {
  root: string;
  active: boolean;
  sessions: SessionEntry[];
  query: string;
  onOpen: (path: string) => void;
  onDelete: (path: string) => void;
  onActivate: (root: string) => void;
  onRemove: (root: string) => void;
}) {
  const [open, setOpen] = useState(active);
  const wasActive = useRef(active);
  // Auto-expand when this workspace becomes active.
  useEffect(() => {
    if (active && !wasActive.current) setOpen(true);
    wasActive.current = active;
  }, [active]);

  const items = useMemo(
    () => filterSessions(sessions, query).slice(0, 12),
    [sessions, query],
  );
  return (
    <div className={`ws-row${active ? " active" : ""}`}>
      <div
        className="ws-head"
        role="button"
        tabIndex={0}
        title={`${root}${active ? " · active" : " · click to make active"}`}
        onClick={() => {
          onActivate(root);
          setOpen((o) => !o);
        }}
        onKeyDown={(e) => e.key === "Enter" && onActivate(root)}
      >
        <span className={`ws-chevron${open ? " open" : ""}`}>
          <ChevronRight size={10} />
        </span>
        <Folder size={13} />
        <span className="ws-name">{wsBasename(root)}</span>
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
      </div>
      {open && (
        <div className="ws-sessions">
          {items.length === 0 ? (
            <div className="sess-empty">No sessions.</div>
          ) : (
            items.map((s) => <SessionRow key={s.path} s={s} onOpen={onOpen} onDelete={onDelete} />)
          )}
        </div>
      )}
    </div>
  );
}

function SessionRow({
  s,
  onOpen,
  onDelete,
}: {
  s: SessionEntry;
  onOpen: (path: string) => void;
  onDelete: (path: string) => void;
}) {
  return (
    <div
      className="session"
      role="button"
      tabIndex={0}
      onClick={() => onOpen(s.path)}
      onKeyDown={(e) => e.key === "Enter" && onOpen(s.path)}
    >
      <div className="sess-preview">{s.firstUserMsg ?? "(empty)"}</div>
      <div className="sess-meta">
        <span>{s.ulid.slice(0, 6)}</span>
        <span>{relTime(Number(s.modifiedMs))}</span>
      </div>
      <button
        className="del"
        title="Delete session"
        onClick={(e) => {
          e.stopPropagation();
          onDelete(s.path);
        }}
      >
        <Trash2 size={11} />
      </button>
    </div>
  );
}

/** Sessions sidebar: pinned search, per-workspace projects, then an
 * "Other" bucket so transcripts from unregistered folders stay visible
 * (Codex hides those — deliberately not copied). */
export function Sidebar({
  sessions,
  workspaces,
  activeWorkspace,
  onOpen,
  onDelete,
  onAddWorkspace,
  onRemoveWorkspace,
  onActivateWorkspace,
}: {
  sessions: SessionEntry[];
  workspaces: string[];
  activeWorkspace: string | null;
  onOpen: (path: string) => void;
  onDelete: (path: string) => void;
  onAddWorkspace: () => void;
  onRemoveWorkspace: (root: string) => void;
  onActivateWorkspace: (root: string | null) => void;
}) {
  const [query, setQuery] = useState("");
  const filtered = useMemo(
    () => filterSessions(sessions, query),
    [sessions, query],
  );
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

  return (
    <div className="sessions">
      {/* Pinned above the scroll area: a search box inside the scroller
          scrolls away and visually collides with the list. */}
      <div className="sess-search">
        <Search size={12} />
        <input
          value={query}
          onChange={(e) => setQuery(e.currentTarget.value)}
          placeholder="Search sessions…"
          spellCheck={false}
        />
      </div>
      <div className="sess-list">
        <div className="ws-section-head">
          <span>Projects</span>
          <button className="mini" title="Add workspace…" onClick={onAddWorkspace}>
            <Plus size={12} />
          </button>
        </div>
        {workspaces.length === 0 && (
          <div className="sess-empty">No workspaces — add a folder.</div>
        )}
        {workspaces.map((root) => (
          <WorkspaceRow
            key={root}
            root={root}
            active={activeWorkspace === root}
            sessions={byWs.m.get(root) ?? []}
            query=""
            onOpen={onOpen}
            onDelete={onDelete}
            onActivate={onActivateWorkspace}
            onRemove={onRemoveWorkspace}
          />
        ))}
        {byWs.other.length > 0 && (
          <>
            <div className="ws-section-head other">
              <span>Other sessions</span>
            </div>
            {byWs.other.slice(0, 20).map((s) => (
              <SessionRow key={s.path} s={s} onOpen={onOpen} onDelete={onDelete} />
            ))}
          </>
        )}
      </div>
    </div>
  );
}
