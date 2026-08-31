import { ArrowDown, FolderGit2, GitCompare, PanelLeft, Search } from "../lib/icons";
import { ContextMeter } from "./ContextMeter";
import { UpdateButton } from "./UpdateButton";
import { WindowControlsMaybe } from "./WindowControls";
import { modLabel } from "../lib/platform";

export function ChatHeader({
  title,
  titleHint,
  diffOpen,
  sidebarOpen,
  onToggleSidebar,
  onPalette,
  onToggleDiff,
}: {
  title: string;
  titleHint?: string;
  diffOpen: boolean;
  sidebarOpen: boolean;
  onToggleSidebar: () => void;
  onPalette: () => void;
  onToggleDiff: () => void;
}) {
  return (
    <header className="chat-head">
      <div className="head-left">
        <button
          type="button"
          className="icon-btn"
          title={
            sidebarOpen ? `Hide sidebar (${modLabel()}B)` : `Show sidebar (${modLabel()}B)`
          }
          onClick={onToggleSidebar}
        >
          <PanelLeft size={13} />
        </button>
        <button type="button" className="icon-btn" title={`Search (${modLabel()}K)`} onClick={onPalette}>
          <Search size={13} />
        </button>
        <div className="chat-title" title={titleHint}>
          <FolderGit2 size={12} className="chat-title-icon" />
          <span>{title}</span>
        </div>
      </div>
      <div className="head-controls">
        <UpdateButton />
        <ContextMeter />
        <button
          className={`icon-btn${diffOpen ? " active" : ""}`}
          title="Review uncommitted git changes vs HEAD"
          onClick={onToggleDiff}
        >
          <GitCompare size={12} />
        </button>
        <WindowControlsMaybe />
      </div>
    </header>
  );
}

export function JumpLatest({ onJump }: { onJump: () => void }) {
  return (
    <button type="button" className="jump-latest" title="Jump to latest" onClick={onJump}>
      <ArrowDown size={16} />
    </button>
  );
}
