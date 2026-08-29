import { FolderGit2, GitCompare, PanelLeft, Search, Settings } from "lucide-react";
import { ContextMeter } from "./ContextMeter";
import { UpdateButton } from "./UpdateButton";
import { WindowControlsMaybe } from "./WindowControls";
import { LogoMark } from "./LogoMark";
import { isMacPlatform, modLabel } from "../lib/platform";

export function TopBar({
  title,
  titleHint,
  diffOpen,
  sidebarOpen,
  onToggleSidebar,
  onPalette,
  onToggleDiff,
  onSettings,
}: {
  title: string;
  titleHint?: string;
  diffOpen: boolean;
  sidebarOpen: boolean;
  onToggleSidebar: () => void;
  onPalette: () => void;
  onToggleDiff: () => void;
  onSettings: () => void;
}) {
  const isMac = isMacPlatform();

  return (
    <header className="app-topbar" data-tauri-drag-region>
      <div className="topbar-left" data-tauri-drag-region>
        {!isMac && (
          <div className="topbar-brand" title="Z Engine">
            <LogoMark size={15} />
            <span>Z Engine</span>
          </div>
        )}
        <button
          type="button"
          className="icon-btn"
          title={
            sidebarOpen ? `Hide sidebar (${modLabel()}B)` : `Show sidebar (${modLabel()}B)`
          }
          onClick={onToggleSidebar}
        >
          <PanelLeft size={14} strokeWidth={1.8} />
        </button>
        <button
          type="button"
          className="topbar-workspace-pill"
          title={titleHint || "Open quick switcher"}
          onClick={onPalette}
        >
          <FolderGit2 size={13} className="topbar-ws-icon" strokeWidth={1.8} />
          <span className="topbar-ws-name">{title}</span>
        </button>
      </div>

      <div className="topbar-center" data-tauri-drag-region>
        <button
          type="button"
          className="topbar-search-bar"
          title={`Search & commands (${modLabel()}K)`}
          onClick={onPalette}
        >
          <Search size={13} className="topbar-search-icon" strokeWidth={1.8} />
          <span className="topbar-search-text">Search chats, workspaces, commands…</span>
          <kbd className="topbar-search-kbd">{modLabel()}K</kbd>
        </button>
      </div>

      <div className="topbar-right" data-tauri-drag-region>
        <UpdateButton />
        <ContextMeter />
        <button
          type="button"
          className={`icon-btn${diffOpen ? " active" : ""}`}
          title="Review uncommitted git changes vs HEAD"
          onClick={onToggleDiff}
        >
          <GitCompare size={14} strokeWidth={1.8} />
        </button>
        <button
          type="button"
          className="icon-btn"
          title="Settings"
          onClick={onSettings}
        >
          <Settings size={14} strokeWidth={1.8} />
        </button>
        <WindowControlsMaybe />
      </div>
    </header>
  );
}
