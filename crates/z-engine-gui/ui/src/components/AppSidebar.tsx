import { Plus, Settings } from "../lib/icons";
import { LogoMark } from "./LogoMark";
import { Sidebar } from "./Sidebar";
import type { SessionActivity } from "../lib/events";
import type { SessionEntry } from "../lib/util";
import { modLabel } from "../lib/platform";

export function AppSidebar({
  sessions,
  workspaces,
  activeWorkspace,
  activeUlid,
  activity,
  version,
  onNewChat,
  onOpen,
  onDelete,
  onAddWorkspace,
  onRemoveWorkspace,
  onActivateWorkspace,
  onSettings,
}: {
  sessions: SessionEntry[];
  workspaces: string[];
  activeWorkspace: string | null;
  activeUlid: string;
  activity: Record<string, SessionActivity>;
  version?: string;
  onNewChat: () => void;
  onOpen: (path: string, projectRoot?: string | null) => void;
  onDelete: (path: string) => void;
  onAddWorkspace: () => void;
  onRemoveWorkspace: (root: string) => void;
  onActivateWorkspace: (root: string | null) => void;
  onSettings: () => void;
}) {
  return (
    <div className="sidebar-slot">
      <aside className="sidebar">
        {/* Brand Header */}
        <div className="sidebar-top-bar">
          <div className="sidebar-brand-pill">
            <LogoMark size={16} />
            <span className="sidebar-brand-text">Z Engine</span>
          </div>
        </div>

        {/* Primary Action */}
        <button className="sidebar-new-chat-btn" onClick={onNewChat} type="button">
          <span className="btn-left">
            <Plus size={13} strokeWidth={2} />
            <span>New chat</span>
          </span>
          <kbd className="sidebar-kbd">{modLabel()}N</kbd>
        </button>

        {/* Tree & Session Deck */}
        <Sidebar
          sessions={sessions}
          workspaces={workspaces}
          activeWorkspace={activeWorkspace}
          activeUlid={activeUlid}
          activity={activity}
          onOpen={onOpen}
          onDelete={onDelete}
          onAddWorkspace={onAddWorkspace}
          onRemoveWorkspace={onRemoveWorkspace}
          onActivateWorkspace={onActivateWorkspace}
        />

        {/* Footer */}
        <div className="sidebar-footer">
          <button
            className="sidebar-footer-btn"
            title="Open Settings"
            onClick={onSettings}
            type="button"
          >
            <Settings size={13} strokeWidth={1.8} />
            <span className="footer-label">Settings</span>
          </button>
          <span className="footer-version-tag">{version ? `v${version}` : "v1.4.0"}</span>
        </div>
      </aside>
    </div>
  );
}
