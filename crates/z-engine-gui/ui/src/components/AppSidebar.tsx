import { Plus, Settings } from "lucide-react";
import { LogoMark } from "./LogoMark";
import { Sidebar } from "./Sidebar";
import type { SessionEntry } from "../lib/util";

export function AppSidebar({
  sessions,
  workspaces,
  activeWorkspace,
  activeUlid,
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
  version?: string;
  onNewChat: () => void;
  onOpen: (path: string) => void;
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
      <button className="newtask" onClick={onNewChat}>
        <Plus size={13} />
        New chat
      </button>
      <Sidebar
        sessions={sessions}
        workspaces={workspaces}
        activeWorkspace={activeWorkspace}
        activeUlid={activeUlid}
        onOpen={onOpen}
        onDelete={onDelete}
        onAddWorkspace={onAddWorkspace}
        onRemoveWorkspace={onRemoveWorkspace}
        onActivateWorkspace={onActivateWorkspace}
      />
      <button className="side-foot gear" title="Settings" onClick={onSettings}>
        <Settings size={13} />
        Settings
        <span className="side-foot-note">{version ? `v${version}` : "Z Engine"}</span>
      </button>
    </aside>
  );
}
