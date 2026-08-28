import { useMemo, useState } from "react";
import {
  AlertCircle,
  LoaderCircle,
  MessageSquare,
  Search,
  Trash2,
  X,
} from "lucide-react";
import type { SessionActivity } from "../lib/events";
import { filterSessions, type SessionEntry } from "../lib/util";
import { formatSessionTime, humanSessionTitle } from "../lib/sessionList";
import { wsBasename } from "../lib/workspaces";
import { WorkspaceFilter } from "./WorkspaceFilter";

interface TimelineGroup {
  label: string;
  items: SessionEntry[];
}

function groupByTimeline(list: SessionEntry[], now = Date.now()): TimelineGroup[] {
  const today = new Date(now);
  today.setHours(0, 0, 0, 0);
  const todayMs = today.getTime();
  const yesterdayMs = todayMs - 86_400_000;
  const sevenDaysMs = todayMs - 7 * 86_400_000;
  const thirtyDaysMs = todayMs - 30 * 86_400_000;

  const todayItems: SessionEntry[] = [];
  const yesterdayItems: SessionEntry[] = [];
  const prev7Items: SessionEntry[] = [];
  const prev30Items: SessionEntry[] = [];
  const olderItems: SessionEntry[] = [];

  for (const s of list) {
    const t = Number(s.modifiedMs) || 0;
    if (t >= todayMs) {
      todayItems.push(s);
    } else if (t >= yesterdayMs) {
      yesterdayItems.push(s);
    } else if (t >= sevenDaysMs) {
      prev7Items.push(s);
    } else if (t >= thirtyDaysMs) {
      prev30Items.push(s);
    } else {
      olderItems.push(s);
    }
  }

  const groups: TimelineGroup[] = [];
  if (todayItems.length > 0) groups.push({ label: "Today", items: todayItems });
  if (yesterdayItems.length > 0) groups.push({ label: "Yesterday", items: yesterdayItems });
  if (prev7Items.length > 0) groups.push({ label: "Previous 7 Days", items: prev7Items });
  if (prev30Items.length > 0) groups.push({ label: "Previous 30 Days", items: prev30Items });
  if (olderItems.length > 0) groups.push({ label: "Earlier", items: olderItems });

  return groups;
}

function SessionRow({
  s,
  active,
  state,
  showProjectBadge,
  onOpen,
  onDelete,
}: {
  s: SessionEntry;
  active: boolean;
  state: SessionActivity | null;
  showProjectBadge: boolean;
  onOpen: (path: string, projectRoot?: string | null) => void;
  onDelete: (path: string) => void;
}) {
  const unread =
    !active && !state && (s.unreadOutcome === "completed" || s.unreadOutcome === "aborted")
      ? s.unreadOutcome
      : null;
  const title = humanSessionTitle(s.firstUserMsg);
  const timeStr = formatSessionTime(s.modifiedMs);
  const projectName = s.projectRoot ? wsBasename(s.projectRoot) : null;

  return (
    <div
      className={`sess-human-item${active ? " active" : ""}${state ? ` ${state}` : ""}${
        unread ? ` unread unread-${unread}` : ""
      }`}
      role="button"
      tabIndex={0}
      title={
        state === "approval"
          ? `Approval required — ${title}`
          : state === "working"
            ? `Working — ${title}`
            : unread
              ? `${unread === "aborted" ? "Aborted" : "Completed"} — ${title}`
              : title
      }
      onClick={(e) => {
        e.stopPropagation();
        onOpen(s.path, s.projectRoot);
      }}
      onKeyDown={(e) => e.key === "Enter" && onOpen(s.path, s.projectRoot)}
    >
      <div className="sess-item-icon-col">
        {state === "working" ? (
          <LoaderCircle size={13} className="spin sess-state-icon working" />
        ) : state === "approval" ? (
          <AlertCircle size={13} className="sess-state-icon approval" />
        ) : (
          <MessageSquare size={13} className="sess-state-icon" />
        )}
      </div>

      <div className="sess-item-main">
        <div className="sess-item-title-row">
          <span className="sess-item-title">{title}</span>
        </div>
        <div className="sess-item-meta-row">
          <span className="sess-item-time">{timeStr}</span>
          {showProjectBadge && projectName && (
            <span className="sess-item-project" title={s.projectRoot ?? ""}>
              {projectName}
            </span>
          )}
        </div>
      </div>

      <div className="sess-item-actions">
        {unread && (
          <span
            className="sess-unread-dot"
            role="status"
            aria-label={`${unread} — unopened`}
          />
        )}
        <button
          type="button"
          className="sess-item-del-btn"
          title="Delete chat"
          onClick={(e) => {
            e.stopPropagation();
            onDelete(s.path);
          }}
        >
          <Trash2 size={12} />
        </button>
      </div>
    </div>
  );
}

export function Sidebar({
  sessions,
  workspaces,
  activeWorkspace: _activeWorkspace,
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
  const [selectedWs, setSelectedWs] = useState<string | "all">("all");

  const filtered = useMemo(() => {
    let list = filterSessions(sessions, query);
    if (selectedWs !== "all") {
      list = list.filter((s) => s.projectRoot === selectedWs);
    }
    return list;
  }, [sessions, query, selectedWs]);

  const timelineGroups = useMemo(() => groupByTimeline(filtered), [filtered]);

  return (
    <div className="sessions-human-container">
      <div className="sess-search-bar">
        <Search size={12} className="sess-search-icon" />
        <input
          value={query}
          onChange={(e) => setQuery(e.currentTarget.value)}
          placeholder="Search chats…"
          spellCheck={false}
        />
        {query && (
          <button
            type="button"
            className="sess-search-clear"
            onClick={() => setQuery("")}
            title="Clear search"
          >
            <X size={11} />
          </button>
        )}
      </div>

      <WorkspaceFilter
        workspaces={workspaces}
        selectedWs={selectedWs}
        onSelectWs={setSelectedWs}
        onAddWorkspace={onAddWorkspace}
        onRemoveWorkspace={onRemoveWorkspace}
        onActivateWorkspace={onActivateWorkspace}
      />

      <div className="sess-human-list">
        {filtered.length === 0 ? (
          <div className="sess-human-empty">
            <MessageSquare size={20} className="sess-empty-icon" />
            <p>No conversations found</p>
            {query && <span>Try searching for something else</span>}
          </div>
        ) : (
          timelineGroups.map((group) => (
            <div key={group.label} className="sess-timeline-group">
              <div className="sess-timeline-heading">{group.label}</div>
              <div className="sess-timeline-items">
                {group.items.map((s) => (
                  <SessionRow
                    key={s.path}
                    s={s}
                    active={s.ulid === activeUlid}
                    state={activity[s.ulid] ?? null}
                    showProjectBadge={selectedWs === "all" && workspaces.length > 1}
                    onOpen={onOpen}
                    onDelete={onDelete}
                  />
                ))}
              </div>
            </div>
          ))
        )}
      </div>
    </div>
  );
}
