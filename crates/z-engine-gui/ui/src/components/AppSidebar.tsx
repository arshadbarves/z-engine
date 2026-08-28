import { Plus, Settings } from "lucide-react";
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
    <aside className="sidebar">
      <div className="brand">
        <LogoMark size={18} />
        <span>Z Engine</span>
      </div>
      <button className="newtask" onClick={onNewChat} type="button">
        <span className="newtask-left">
          <Plus size={13} />
          <span>New chat</span>
        </span>
        <kbd>{modLabel()}N</kbd>
      </button>
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
      <button className="side-foot gear" title="Settings" onClick={onSettings} type="button">
        <span className="side-foot-left">
          <Settings size={13} />
          <span>Settings</span>
        </span>
        <span className="side-foot-note">{version ? `v${version}` : "Z Engine"}</span>
      </button>
    </aside>
  );
}
